use core::{any::TypeId, fmt, marker::PhantomData};

use rushdown::ast::{
    Arena, KindData, Link, LinkKind, NodeKind, NodeRef, NodeType, Paragraph, PrettyPrint, Text,
    WalkStatus, pp_indent,
};
use rushdown::renderer::{
    self, BoxRenderNode, NoRendererOptions, NodeRenderer, NodeRendererRegistry, RenderNode,
    TextWrite,
};
use rushdown::{
    as_kind_data, as_kind_data_mut, as_type_data, as_type_data_mut, matches_kind,
    new_markdown_to_html_string,
    parser::{self, AnyAstTransformer, AstTransformer, ParserExtension, parser_extension},
    renderer::html::{self, RendererExtension, renderer_extension},
    text::{Reader, Segment},
    util::{EscapeUrlOptions, escape_html, escape_url},
};
use rushdown_diagram::{
    DiagramHtmlRendererOptions, DiagramParserOptions, diagram_html_renderer_extension,
    diagram_parser_extension,
};
use rushdown_footnote::{
    FootnoteHtmlRendererOptions, footnote_html_renderer_extension, footnote_parser_extension,
};
use rushdown_highlighting::{
    HighlightingHtmlRendererOptions, HighlightingMode, highlighting_html_renderer_extension,
};
use rushdown_link_attribute::link_attribute_parser_extension;
use yaml_rust::YamlLoader;

const EXTERNAL_LINK_ICON: &str = r#"<svg class="external-icon" viewBox="0 0 512 512"><path d="M320 0H288V64h32 82.7L201.4 265.4 178.7 288 224 333.3l22.6-22.6L448 109.3V192v32h64V192 32 0H480 320zM32 32H0V64 480v32H32 456h32V480 352 320H424v32 96H64V96h96 32V32H160 32z"></path></svg>"#;

pub trait MarkdownHtmlRenderer {
    fn render_html(&self, markdown: &str) -> rushdown::Result<String>;
}

pub fn title_from_markdown(markdown: &str) -> Option<String> {
    frontmatter_title(markdown).or_else(|| markdown_heading_title(markdown).map(str::to_string))
}

pub fn tags_from_markdown(markdown: &str) -> Vec<String> {
    let Some(frontmatter) = frontmatter_yaml(markdown) else {
        return Vec::new();
    };
    let tags_key = yaml_rust::Yaml::String("tags".to_string());
    let Some(tags) = frontmatter.as_hash().and_then(|hash| hash.get(&tags_key)) else {
        return Vec::new();
    };

    match tags {
        yaml_rust::Yaml::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str())
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(str::to_string)
            .collect(),
        yaml_rust::Yaml::String(value) => value
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

pub fn markdown_without_frontmatter(markdown: &str) -> &str {
    let Some(rest) = markdown.strip_prefix("---") else {
        return markdown;
    };
    let Some(rest) = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"))
    else {
        return markdown;
    };
    let mut offset = markdown.len() - rest.len();
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim();
        offset += line.len();
        if trimmed == "---" {
            return &markdown[offset..];
        }
    }

    markdown
}

fn frontmatter_title(markdown: &str) -> Option<String> {
    let title_key = yaml_rust::Yaml::String("title".to_string());
    let frontmatter = frontmatter_yaml(markdown)?;
    let title = frontmatter.as_hash()?.get(&title_key)?;
    title
        .as_str()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_string)
}

fn frontmatter_yaml(markdown: &str) -> Option<yaml_rust::Yaml> {
    let mut lines = markdown.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }

    let mut yaml = String::new();
    for line in lines {
        if line.trim() == "---" {
            return YamlLoader::load_from_str(&yaml).ok()?.into_iter().next();
        }

        yaml.push_str(line);
        yaml.push('\n');
    }

    None
}

fn markdown_heading_title(markdown: &str) -> Option<&str> {
    markdown.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("# ")
            .map(str::trim)
            .filter(|title| !title.is_empty())
    })
}

#[derive(Debug, Clone)]
pub struct RushdownMarkdownRenderer {
    parser_options: parser::Options,
    renderer_options: html::Options,
}

impl Default for RushdownMarkdownRenderer {
    fn default() -> Self {
        Self {
            parser_options: parser::Options {
                attributes: true,
                auto_heading_ids: true,
                ..parser::Options::default()
            },
            renderer_options: html::Options::default(),
        }
    }
}

impl RushdownMarkdownRenderer {
    pub fn new() -> Self {
        Self::with_options(
            parser::Options {
                attributes: true,
                auto_heading_ids: true,
                ..parser::Options::default()
            },
            html::Options::default(),
        )
    }

    pub fn with_options(parser_options: parser::Options, renderer_options: html::Options) -> Self {
        Self {
            parser_options,
            renderer_options,
        }
    }

    fn parser_extensions(&self) -> impl ParserExtension {
        rushdown_meta::meta_parser_extension(rushdown_meta::MetaParserOptions::default())
            .and(footnote_parser_extension())
            .and(diagram_parser_extension(DiagramParserOptions::default()))
            .and(link_attribute_parser_extension())
            .and(callout_parser_extension())
            .and(parser::gfm_table())
            .and(parser::gfm_task_list_item())
    }

    fn renderer_extensions(&self) -> impl RendererExtension<'_> {
        footnote_html_renderer_extension(FootnoteHtmlRendererOptions::default())
            .and(diagram_html_renderer_extension(
                DiagramHtmlRendererOptions::default(),
            ))
            .and(highlighting_html_renderer_extension(
                HighlightingHtmlRendererOptions {
                    mode: HighlightingMode::Attribute,
                    ..HighlightingHtmlRendererOptions::default()
                },
            ))
            .and(callout_html_renderer_extension())
            .and(classified_link_html_renderer_extension())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CalloutFold {
    None,
    Expanded,
    Collapsed,
}

#[derive(Debug)]
struct Callout {
    kind: String,
    fold: CalloutFold,
}

impl Callout {
    fn new(kind: String, fold: CalloutFold) -> Self {
        Self { kind, fold }
    }
}

impl NodeKind for Callout {
    fn typ(&self) -> NodeType {
        NodeType::ContainerBlock
    }

    fn kind_name(&self) -> &'static str {
        "Callout"
    }
}

impl PrettyPrint for Callout {
    fn pretty_print(&self, w: &mut dyn fmt::Write, _source: &str, level: usize) -> fmt::Result {
        writeln!(w, "{}Callout: kind={}", pp_indent(level), self.kind)
    }
}

impl From<Callout> for KindData {
    fn from(callout: Callout) -> Self {
        KindData::Extension(Box::new(callout))
    }
}

#[derive(Debug)]
struct CalloutTitle {
    fold: CalloutFold,
}

impl CalloutTitle {
    fn new(fold: CalloutFold) -> Self {
        Self { fold }
    }
}

impl NodeKind for CalloutTitle {
    fn typ(&self) -> NodeType {
        NodeType::ContainerBlock
    }

    fn kind_name(&self) -> &'static str {
        "CalloutTitle"
    }
}

impl PrettyPrint for CalloutTitle {
    fn pretty_print(&self, w: &mut dyn fmt::Write, _source: &str, level: usize) -> fmt::Result {
        writeln!(w, "{}CalloutTitle", pp_indent(level))
    }
}

impl From<CalloutTitle> for KindData {
    fn from(title: CalloutTitle) -> Self {
        KindData::Extension(Box::new(title))
    }
}

#[derive(Debug)]
struct CalloutContent;

impl CalloutContent {
    fn new() -> Self {
        Self
    }
}

impl NodeKind for CalloutContent {
    fn typ(&self) -> NodeType {
        NodeType::ContainerBlock
    }

    fn kind_name(&self) -> &'static str {
        "CalloutContent"
    }
}

impl PrettyPrint for CalloutContent {
    fn pretty_print(&self, w: &mut dyn fmt::Write, _source: &str, level: usize) -> fmt::Result {
        writeln!(w, "{}CalloutContent", pp_indent(level))
    }
}

impl From<CalloutContent> for KindData {
    fn from(content: CalloutContent) -> Self {
        KindData::Extension(Box::new(content))
    }
}

#[derive(Debug)]
struct CalloutAstTransformer;

impl CalloutAstTransformer {
    fn new() -> Self {
        Self
    }
}

impl AstTransformer for CalloutAstTransformer {
    fn transform(
        &self,
        arena: &mut Arena,
        doc_ref: NodeRef,
        reader: &mut rushdown::text::BasicReader,
        _context: &mut parser::Context,
    ) {
        let mut blockquotes = Vec::new();
        rushdown::ast::walk(
            arena,
            doc_ref,
            &mut |arena: &Arena,
                  node_ref: NodeRef,
                  entering: bool|
             -> rushdown::Result<WalkStatus> {
                if entering && matches_kind!(arena, node_ref, Blockquote) {
                    blockquotes.push(node_ref);
                }
                Ok(WalkStatus::Continue)
            },
        )
        .unwrap();

        for blockquote_ref in blockquotes {
            let Some(first_child_ref) = arena[blockquote_ref].first_child() else {
                continue;
            };
            if !matches_kind!(arena, first_child_ref, Paragraph) {
                continue;
            }

            let (marker, pos, title_line_stop, remaining_title_paragraph_lines) = {
                let block_data = as_type_data!(arena, first_child_ref, Block);
                let Some(first_line) = block_data.source().first() else {
                    continue;
                };
                let first_line_text = first_line.str(reader.source());
                let Some(marker) = parse_callout_marker(&first_line_text) else {
                    continue;
                };
                (
                    marker,
                    first_line.start(),
                    first_line.start() + first_line_text.trim_end_matches(['\r', '\n']).len(),
                    block_data
                        .source()
                        .iter()
                        .skip(1)
                        .copied()
                        .collect::<Vec<_>>(),
                )
            };
            let Some(parent_ref) = arena[blockquote_ref].parent() else {
                continue;
            };

            let continued_paragraph_ref = prepare_callout_title_paragraph(
                arena,
                first_child_ref,
                reader.source(),
                pos + marker.title_start,
                title_line_stop,
                remaining_title_paragraph_lines,
                default_callout_title(&marker.kind),
            );

            let callout_ref = arena.new_node(Callout::new(marker.kind, marker.fold));
            let title_ref = arena.new_node(CalloutTitle::new(marker.fold));
            let content_ref = arena.new_node(CalloutContent::new());
            title_ref.append_child(arena, first_child_ref);
            if let Some(continued_paragraph_ref) = continued_paragraph_ref {
                content_ref.append_child(arena, continued_paragraph_ref);
            }
            let body_children = arena[blockquote_ref].children(arena).collect::<Vec<_>>();
            for child_ref in body_children {
                content_ref.append_child(arena, child_ref);
            }
            callout_ref.append_child(arena, title_ref);
            callout_ref.append_child(arena, content_ref);
            arena[callout_ref].set_pos(pos);
            parent_ref.replace_child(arena, blockquote_ref, callout_ref);
        }
    }
}

impl From<CalloutAstTransformer> for AnyAstTransformer {
    fn from(transformer: CalloutAstTransformer) -> Self {
        AnyAstTransformer::Extension(Box::new(transformer))
    }
}

#[derive(Debug)]
struct CalloutMarker {
    kind: String,
    fold: CalloutFold,
    title_start: usize,
}

fn parse_callout_marker(line: &str) -> Option<CalloutMarker> {
    let trimmed = line.trim_start();
    let leading_spaces = line.len() - trimmed.len();
    let marker = trimmed.strip_prefix("[!")?;
    let close_index = marker.find(']')?;
    let raw_kind = &marker[..close_index];
    let kind = canonical_callout_kind(raw_kind)?;
    let after_marker_start = leading_spaces + 2 + close_index + 1;
    let after_marker = &line[after_marker_start..];
    let after_marker = after_marker.trim_start();
    let (fold, after_fold) = match after_marker.as_bytes().first().copied() {
        Some(b'+') => (CalloutFold::Expanded, &after_marker[1..]),
        Some(b'-') => (CalloutFold::Collapsed, &after_marker[1..]),
        _ => (CalloutFold::None, after_marker),
    };
    let title = after_fold.trim_start();
    let title_start = line.len() - title.len();

    Some(CalloutMarker {
        kind,
        fold,
        title_start,
    })
}

fn prepare_callout_title_paragraph(
    arena: &mut Arena,
    paragraph_ref: NodeRef,
    source: &str,
    title_start: usize,
    title_line_stop: usize,
    remaining_lines: Vec<Segment>,
    default_title: &str,
) -> Option<NodeRef> {
    as_type_data_mut!(arena, paragraph_ref, Block)
        .put_back_source(vec![Segment::new(title_start, title_line_stop)]);

    let content_paragraph_ref = if remaining_lines.is_empty() {
        None
    } else {
        let paragraph_ref = arena.new_node(Paragraph::new());
        {
            let block = as_type_data_mut!(arena, paragraph_ref, Block);
            for line in remaining_lines {
                block.append_source_line(line);
            }
        }
        Some(paragraph_ref)
    };

    let children = arena[paragraph_ref].children(arena).collect::<Vec<_>>();

    for child_ref in children {
        let remove_child = if matches_kind!(arena, child_ref, Text) {
            let mut remove_text = false;
            let index = as_kind_data!(arena, child_ref, Text).index().copied();
            if let Some(index) = index {
                if index.stop() <= title_start {
                    remove_text = true;
                } else if index.start() < title_start {
                    let stop = index.stop();
                    as_kind_data_mut!(arena, child_ref, Text).set(Segment::new(title_start, stop));
                    remove_text = as_kind_data!(arena, child_ref, Text)
                        .str(source)
                        .trim()
                        .is_empty();
                }
            } else {
                remove_text = as_kind_data!(arena, child_ref, Text)
                    .str(source)
                    .trim()
                    .is_empty();
            }

            if !remove_text {
                let index = as_kind_data!(arena, child_ref, Text).index().copied();
                if let Some(index) = index {
                    if index.start() >= title_line_stop {
                        if let Some(content_paragraph_ref) = content_paragraph_ref {
                            content_paragraph_ref.append_child(arena, child_ref);
                        } else {
                            remove_text = true;
                        }
                    } else if index.stop() > title_line_stop {
                        if let Some(content_paragraph_ref) = content_paragraph_ref {
                            let continued_text_ref = arena
                                .new_node(Text::new(Segment::new(title_line_stop, index.stop())));
                            content_paragraph_ref.append_child(arena, continued_text_ref);
                            as_kind_data_mut!(arena, child_ref, Text)
                                .set(Segment::new(index.start(), title_line_stop));
                        }
                        remove_text = as_kind_data!(arena, child_ref, Text)
                            .str(source)
                            .trim()
                            .is_empty();
                    }
                }
            }

            if !remove_text {
                let text = as_kind_data!(arena, child_ref, Text)
                    .str(source)
                    .to_string();
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    remove_text = true;
                } else if trimmed.len() != text.len() {
                    as_kind_data_mut!(arena, child_ref, Text).set(trimmed.to_string());
                }
            }

            remove_text
        } else {
            match arena[child_ref].pos() {
                Some(pos) if pos < title_start => true,
                Some(pos) if pos >= title_line_stop => {
                    if let Some(content_paragraph_ref) = content_paragraph_ref {
                        content_paragraph_ref.append_child(arena, child_ref);
                    }
                    false
                }
                _ => false,
            }
        };

        if remove_child {
            child_ref.remove(arena);
        }
    }

    if arena[paragraph_ref].first_child().is_none() {
        let title_ref = arena.new_node(Text::new(default_title));
        paragraph_ref.append_child(arena, title_ref);
    }

    content_paragraph_ref.filter(|content_ref| arena[*content_ref].first_child().is_some())
}

fn canonical_callout_kind(kind: &str) -> Option<String> {
    let kind = kind.trim().to_ascii_lowercase();
    let canonical = match kind.as_str() {
        "note" => "note",
        "abstract" | "summary" | "tldr" => "abstract",
        "info" => "info",
        "todo" => "todo",
        "tip" | "hint" | "important" => "tip",
        "success" | "check" | "done" => "success",
        "question" | "help" | "faq" => "question",
        "warning" | "caution" | "attention" => "warning",
        "failure" | "fail" | "missing" => "failure",
        "danger" | "error" => "danger",
        "bug" => "bug",
        "example" => "example",
        "quote" | "cite" => "quote",
        _ => return None,
    };
    Some(canonical.to_string())
}

fn default_callout_title(kind: &str) -> &str {
    match kind {
        "abstract" => "Abstract",
        "info" => "Info",
        "todo" => "Todo",
        "tip" => "Tip",
        "success" => "Success",
        "question" => "Question",
        "warning" => "Warning",
        "failure" => "Failure",
        "danger" => "Danger",
        "bug" => "Bug",
        "example" => "Example",
        "quote" => "Quote",
        _ => "Note",
    }
}

struct CalloutHtmlRenderer<W: TextWrite> {
    writer: html::Writer,
    _phantom: PhantomData<*const W>,
}

impl<W: TextWrite> CalloutHtmlRenderer<W> {
    fn new(format_options: html::Options) -> Self {
        Self {
            writer: html::Writer::with_options(format_options),
            _phantom: PhantomData,
        }
    }
}

impl<W: TextWrite> RenderNode<W> for CalloutHtmlRenderer<W> {
    fn render_node<'a>(
        &self,
        w: &mut W,
        _source: &'a str,
        arena: &'a Arena,
        node_ref: NodeRef,
        entering: bool,
        _context: &mut renderer::Context,
    ) -> rushdown::Result<WalkStatus> {
        let KindData::Extension(callout_data) = arena[node_ref].kind_data() else {
            unreachable!("registered Callout renderer should only receive extension nodes");
        };
        let callout = callout_data
            .as_any()
            .downcast_ref::<Callout>()
            .expect("registered Callout renderer should only receive Callout nodes");

        if entering {
            self.writer.write_safe_str(w, "<div class=\"callout")?;
            match callout.fold {
                CalloutFold::Expanded => {
                    self.writer.write_safe_str(w, " is-collapsible")?;
                }
                CalloutFold::Collapsed => {
                    self.writer
                        .write_safe_str(w, " is-collapsible is-collapsed")?;
                }
                CalloutFold::None => {}
            }
            self.writer.write_safe_str(w, "\" data-callout=\"")?;
            self.writer.write_html(w, &callout.kind)?;
            self.writer.write_safe_str(w, "\">")?;
        } else {
            self.writer.write_safe_str(w, "</div>")?;
        }

        Ok(WalkStatus::Continue)
    }
}

impl<'r, W> NodeRenderer<'r, W> for CalloutHtmlRenderer<W>
where
    W: TextWrite + 'r,
{
    fn register_node_renderer_fn(self, registry: &mut impl NodeRendererRegistry<'r, W>) {
        registry.register_node_renderer_fn(TypeId::of::<Callout>(), BoxRenderNode::new(self));
    }
}

struct CalloutTitleHtmlRenderer<W: TextWrite> {
    writer: html::Writer,
    _phantom: PhantomData<*const W>,
}

impl<W: TextWrite> CalloutTitleHtmlRenderer<W> {
    fn new(format_options: html::Options) -> Self {
        Self {
            writer: html::Writer::with_options(format_options),
            _phantom: PhantomData,
        }
    }
}

impl<W: TextWrite> RenderNode<W> for CalloutTitleHtmlRenderer<W> {
    fn render_node<'a>(
        &self,
        w: &mut W,
        _source: &'a str,
        arena: &'a Arena,
        node_ref: NodeRef,
        entering: bool,
        _context: &mut renderer::Context,
    ) -> rushdown::Result<WalkStatus> {
        let KindData::Extension(title_data) = arena[node_ref].kind_data() else {
            unreachable!("registered CalloutTitle renderer should only receive extension nodes");
        };
        let title = title_data
            .as_any()
            .downcast_ref::<CalloutTitle>()
            .expect("registered CalloutTitle renderer should only receive CalloutTitle nodes");

        if entering {
            self.writer.write_safe_str(
                w,
                "<div class=\"callout-title\"><div class=\"callout-icon\"></div>",
            )?;
            if title.fold != CalloutFold::None {
                let expanded = if title.fold == CalloutFold::Collapsed {
                    "false"
                } else {
                    "true"
                };
                self.writer.write_safe_str(w, "<button class=\"fold-callout-icon\" type=\"button\" aria-label=\"Toggle callout\" aria-expanded=\"")?;
                self.writer.write_safe_str(w, expanded)?;
                self.writer.write_safe_str(w, "\"></button>")?;
            }
            self.writer
                .write_safe_str(w, "<div class=\"callout-title-inner\">")?;
        } else {
            self.writer.write_safe_str(w, "</div></div>")?;
        }

        Ok(WalkStatus::Continue)
    }
}

impl<'r, W> NodeRenderer<'r, W> for CalloutTitleHtmlRenderer<W>
where
    W: TextWrite + 'r,
{
    fn register_node_renderer_fn(self, registry: &mut impl NodeRendererRegistry<'r, W>) {
        registry.register_node_renderer_fn(TypeId::of::<CalloutTitle>(), BoxRenderNode::new(self));
    }
}

struct CalloutContentHtmlRenderer<W: TextWrite> {
    writer: html::Writer,
    _phantom: PhantomData<*const W>,
}

impl<W: TextWrite> CalloutContentHtmlRenderer<W> {
    fn new(format_options: html::Options) -> Self {
        Self {
            writer: html::Writer::with_options(format_options),
            _phantom: PhantomData,
        }
    }
}

impl<W: TextWrite> RenderNode<W> for CalloutContentHtmlRenderer<W> {
    fn render_node<'a>(
        &self,
        w: &mut W,
        _source: &'a str,
        _arena: &'a Arena,
        _node_ref: NodeRef,
        entering: bool,
        _context: &mut renderer::Context,
    ) -> rushdown::Result<WalkStatus> {
        if entering {
            self.writer
                .write_safe_str(w, "<div class=\"callout-content\"><div>")?;
        } else {
            self.writer.write_safe_str(w, "</div></div>")?;
        }

        Ok(WalkStatus::Continue)
    }
}

impl<'r, W> NodeRenderer<'r, W> for CalloutContentHtmlRenderer<W>
where
    W: TextWrite + 'r,
{
    fn register_node_renderer_fn(self, registry: &mut impl NodeRendererRegistry<'r, W>) {
        registry
            .register_node_renderer_fn(TypeId::of::<CalloutContent>(), BoxRenderNode::new(self));
    }
}

fn callout_parser_extension() -> impl ParserExtension {
    parser_extension(|parser| {
        parser.add_ast_transformer(CalloutAstTransformer::new, parser::NoParserOptions, 100);
    })
}

fn callout_html_renderer_extension<'r, W>() -> impl RendererExtension<'r, W>
where
    W: TextWrite + 'r,
{
    renderer_extension(|renderer| {
        renderer.add_node_renderer(CalloutHtmlRenderer::new, NoRendererOptions);
        renderer.add_node_renderer(CalloutTitleHtmlRenderer::new, NoRendererOptions);
        renderer.add_node_renderer(CalloutContentHtmlRenderer::new, NoRendererOptions);
    })
}

struct ClassifiedLinkHtmlRenderer<W: TextWrite> {
    format_options: html::Options,
    writer: html::Writer,
    _phantom: PhantomData<*const W>,
}

impl<W: TextWrite> ClassifiedLinkHtmlRenderer<W> {
    fn new(format_options: html::Options) -> Self {
        Self {
            writer: html::Writer::with_options(format_options.clone()),
            format_options,
            _phantom: PhantomData,
        }
    }

    fn write_attributes<'a>(
        &self,
        w: &mut W,
        source: &'a str,
        arena: &'a Arena,
        node_ref: NodeRef,
        skip_data_slug: bool,
    ) -> rushdown::Result<()> {
        let valid = self
            .format_options
            .attribute_filters
            .as_ref()
            .and_then(|filters| filters.link());

        for (key, value) in arena[node_ref].attributes() {
            if key == "class" || (skip_data_slug && key == "data-slug") {
                continue;
            }
            if !key.starts_with("data-") && !key.starts_with("aria-") {
                if let Some(valid_set) = valid {
                    if !valid_set.contains(key) {
                        continue;
                    }
                }
            }

            self.writer.write_safe_str(w, " ")?;
            w.write_str(key.as_str())?;
            self.writer.write_safe_str(w, "=\"")?;
            self.writer.raw_write(w, value.str(source).as_ref())?;
            self.writer.write_safe_str(w, "\"")?;
        }

        Ok(())
    }

    fn write_link_open<'a>(
        &self,
        w: &mut W,
        source: &'a str,
        arena: &'a Arena,
        node_ref: NodeRef,
        link: &'a Link,
        classification: LinkClassification,
    ) -> rushdown::Result<()> {
        let mut dest = escape_url(
            link.destination().bytes(source),
            &EscapeUrlOptions {
                resolves_refs: !matches!(link.link_kind(), LinkKind::Auto(_)),
                ..EscapeUrlOptions::for_url()
            },
        );

        self.writer.write_safe_str(w, "<a href=\"")?;
        if self.format_options.allows_unsafe || !html::is_dangerous_url(&dest) {
            dest = escape_html(dest);
            w.write_str(String::from_utf8_lossy(&dest).as_ref())?;
        }
        self.writer.write_safe_str(w, "\"")?;

        if let Some(title) = link.title() {
            self.writer.write_safe_str(w, " title=\"")?;
            self.writer.raw_write(w, title.str(source).as_ref())?;
            self.writer.write_safe_str(w, "\"")?;
        }

        let existing_class = arena[node_ref]
            .attributes()
            .get("class")
            .map(|value| value.str(source));
        match classification {
            LinkClassification::Plain => {
                if let Some(existing_class) =
                    existing_class.as_deref().filter(|class| !class.is_empty())
                {
                    self.writer.write_safe_str(w, " class=\"")?;
                    self.writer.raw_write(w, existing_class)?;
                    self.writer.write_safe_str(w, "\"")?;
                }
            }
            LinkClassification::External => {
                self.writer.write_safe_str(w, " class=\"external")?;
                if let Some(existing_class) =
                    existing_class.as_deref().filter(|class| !class.is_empty())
                {
                    self.writer.write_safe_str(w, " ")?;
                    self.writer.raw_write(w, existing_class)?;
                }
                self.writer.write_safe_str(w, "\"")?;
            }
            LinkClassification::Internal { .. } => {
                self.writer.write_safe_str(w, " class=\"internal alias")?;
                if let Some(existing_class) =
                    existing_class.as_deref().filter(|class| !class.is_empty())
                {
                    self.writer.write_safe_str(w, " ")?;
                    self.writer.raw_write(w, existing_class)?;
                }
                self.writer.write_safe_str(w, "\"")?;
            }
        }

        if let LinkClassification::Internal { ref slug } = classification {
            self.writer.write_safe_str(w, " data-slug=\"")?;
            self.writer.raw_write(w, &slug)?;
            self.writer.write_safe_str(w, "\"")?;
        }

        self.write_attributes(
            w,
            source,
            arena,
            node_ref,
            matches!(classification, LinkClassification::Internal { .. }),
        )?;
        self.writer.write_safe_str(w, ">")?;
        Ok(())
    }
}

impl<W: TextWrite> RenderNode<W> for ClassifiedLinkHtmlRenderer<W> {
    fn render_node<'a>(
        &self,
        w: &mut W,
        source: &'a str,
        arena: &'a Arena,
        node_ref: NodeRef,
        entering: bool,
        _context: &mut renderer::Context,
    ) -> rushdown::Result<WalkStatus> {
        let link = as_kind_data!(arena, node_ref, Link);
        let classification = if link_has_image_child(arena, node_ref) {
            LinkClassification::Plain
        } else {
            classify_link(link.destination_str(source))
        };

        if entering {
            self.write_link_open(w, source, arena, node_ref, link, classification.clone())?;
            if matches!(link.link_kind(), LinkKind::Auto(_)) {
                if let Some(first_child) = arena[node_ref].first_child() {
                    if matches_kind!(arena, first_child, Text) {
                        let text = as_kind_data!(arena, first_child, Text);
                        self.writer.raw_write(w, text.str(source))?;
                        if matches!(classification, LinkClassification::External) {
                            self.writer.write_safe_str(w, EXTERNAL_LINK_ICON)?;
                        }
                        self.writer.write_safe_str(w, "</a>")?;
                    }
                }
                return Ok(WalkStatus::SkipChildren);
            }
        } else {
            if matches!(classification, LinkClassification::External) {
                self.writer.write_safe_str(w, EXTERNAL_LINK_ICON)?;
            }
            self.writer.write_safe_str(w, "</a>")?;
        }

        Ok(WalkStatus::Continue)
    }
}

impl<'r, W> NodeRenderer<'r, W> for ClassifiedLinkHtmlRenderer<W>
where
    W: TextWrite + 'r,
{
    fn register_node_renderer_fn(self, registry: &mut impl NodeRendererRegistry<'r, W>) {
        registry.register_node_renderer_fn(TypeId::of::<Link>(), BoxRenderNode::new(self));
    }
}

fn classified_link_html_renderer_extension<'r, W>() -> impl RendererExtension<'r, W>
where
    W: TextWrite + 'r,
{
    renderer_extension(|renderer| {
        renderer.add_node_renderer(ClassifiedLinkHtmlRenderer::new, NoRendererOptions)
    })
}

#[derive(Clone, Debug)]
enum LinkClassification {
    Internal { slug: String },
    External,
    Plain,
}

fn classify_link(destination: &str) -> LinkClassification {
    if is_external_link(destination) {
        LinkClassification::External
    } else {
        LinkClassification::Internal {
            slug: internal_slug(destination),
        }
    }
}

fn is_external_link(destination: &str) -> bool {
    let Some(colon_index) = destination.find(':') else {
        return false;
    };
    let scheme = &destination[..colon_index];
    !scheme.is_empty()
        && scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'.' | b'-'))
}

fn internal_slug(destination: &str) -> String {
    let path = destination
        .split_once(['?', '#'])
        .map_or(destination, |(path, _)| path);
    let had_trailing_slash = path.ends_with('/');
    let slug = path
        .trim_start_matches("./")
        .trim_start_matches('/')
        .trim_end_matches('/')
        .strip_suffix(".md")
        .unwrap_or_else(|| {
            path.trim_start_matches("./")
                .trim_start_matches('/')
                .trim_end_matches('/')
        });

    if slug.is_empty() || slug == "." {
        "index".to_string()
    } else if had_trailing_slash {
        format!("{slug}/index")
    } else {
        slug.to_string()
    }
}

fn link_has_image_child(arena: &Arena, node_ref: NodeRef) -> bool {
    arena[node_ref]
        .children(arena)
        .any(|child_ref| matches_kind!(arena, child_ref, Image))
}

impl MarkdownHtmlRenderer for RushdownMarkdownRenderer {
    fn render_html(&self, markdown: &str) -> rushdown::Result<String> {
        let render = new_markdown_to_html_string(
            self.parser_options.clone(),
            self.renderer_options.clone(),
            self.parser_extensions(),
            self.renderer_extensions(),
        );
        let mut html = String::new();
        render(&mut html, markdown)?;
        Ok(html)
    }
}

#[cfg(test)]
mod tests {
    use super::{MarkdownHtmlRenderer, RushdownMarkdownRenderer, title_from_markdown};

    #[test]
    fn title_from_markdown_uses_frontmatter_title_before_heading() {
        let markdown = "---\ntitle: example\n---\n\n# Heading\n\nBody";

        let title = title_from_markdown(markdown);

        assert_eq!(title.as_deref(), Some("example"));
    }

    #[test]
    fn title_from_markdown_falls_back_to_first_heading() {
        let markdown = "# Heading\n\nBody";

        let title = title_from_markdown(markdown);

        assert_eq!(title.as_deref(), Some("Heading"));
    }

    #[test]
    fn tags_from_markdown_reads_frontmatter_array() {
        let markdown = "---\ntags:\n  - rust\n  - notes\n---\n\n# Heading";

        let tags = super::tags_from_markdown(markdown);

        assert_eq!(tags, vec!["rust", "notes"]);
    }

    #[test]
    fn highlights_fenced_code_blocks() {
        let html = RushdownMarkdownRenderer::new()
            .render_html("```rust\nlet a = 10;\n```")
            .expect("markdown should render");

        assert!(html.contains("<pre"), "{html}");
        assert!(html.contains("<code"), "{html}");
        assert!(html.contains("style=\""), "{html}");
        assert!(html.contains("let"));
        assert!(html.contains("10"));
    }

    #[test]
    fn renders_footnotes() {
        let html = RushdownMarkdownRenderer::new()
            .render_html(
                r#"
That's some text with a footnote.[^1]

[^1]: And that's the footnote.
"#,
            )
            .expect("markdown should render");

        assert!(html.contains("footnote"), "{html}");
        assert!(html.contains("And that's the footnote."), "{html}");
        assert!(html.contains("That's some text with a footnote."), "{html}");
    }

    #[test]
    fn renders_mermaid_diagrams() {
        let html = RushdownMarkdownRenderer::new()
            .render_html(
                r#"
```mermaid
graph LR
    A --- B
    B-->C[fa:fa-ban forbidden]
    B-->D(fa:fa-spinner);
```
"#,
            )
            .expect("markdown should render");

        assert!(html.contains("mermaid"), "{html}");
        assert!(html.contains("graph LR"), "{html}");
        assert!(html.contains("A --- B"), "{html}");
    }

    #[test]
    fn renders_callouts() {
        let html = RushdownMarkdownRenderer::new()
            .render_html(
                r#"
> [!info] Title
>
> This is a callout!
"#,
            )
            .expect("markdown should render");

        assert!(
            html.contains(r#"<div class="callout" data-callout="info">"#),
            "{html}"
        );
        assert!(
            html.contains(r#"<div class="callout-icon"></div>"#),
            "{html}"
        );
        assert!(html.contains(r#"<p>Title</p>"#), "{html}");
        assert!(
            html.contains(r#"<div class="callout-content"><div>"#),
            "{html}"
        );
        assert!(html.contains("<p>This is a callout!</p>"), "{html}");
        assert!(!html.contains("[!info]"), "{html}");
    }

    #[test]
    fn renders_callout_aliases_as_canonical_types() {
        let html = RushdownMarkdownRenderer::new()
            .render_html(
                r#"
> [!summary]
>
> Summary body.

> [!cite] Citation
>
> Quote body.
"#,
            )
            .expect("markdown should render");

        assert!(html.contains(r#"data-callout="abstract""#), "{html}");
        assert!(html.contains("<p>Abstract</p>"), "{html}");
        assert!(html.contains(r#"data-callout="quote""#), "{html}");
        assert!(html.contains("<p>Citation</p>"), "{html}");
    }

    #[test]
    fn renders_markdown_in_callout_titles() {
        let html = RushdownMarkdownRenderer::new()
            .render_html(
                r#"
> [!quote] [**Jack Kornfield**](https://jackkornfield.com/bio/)
>
> The trouble is, you think you have time.
"#,
            )
            .expect("markdown should render");

        assert!(
            html.contains(
                r#"<p><a href="https://jackkornfield.com/bio/" class="external"><strong>Jack Kornfield</strong><svg class="external-icon""#
            ),
            "{html}"
        );
        assert!(!html.contains("[**Jack Kornfield**]"), "{html}");
    }

    #[test]
    fn renders_collapsible_callouts() {
        let html = RushdownMarkdownRenderer::new()
            .render_html(
                r#"
> [!warning]- Careful
>
> Hidden by default.

> [!tip]+ Expanded
>
> Visible by default.
"#,
            )
            .expect("markdown should render");

        assert!(
            html.contains(
                r#"<div class="callout is-collapsible is-collapsed" data-callout="warning">"#
            ),
            "{html}"
        );
        assert!(html.contains(r#"aria-expanded="false""#), "{html}");
        assert!(
            html.contains(r#"<div class="callout is-collapsible" data-callout="tip">"#),
            "{html}"
        );
        assert!(html.contains(r#"aria-expanded="true""#), "{html}");
        assert!(html.contains(r#"class="fold-callout-icon""#), "{html}");
    }

    #[test]
    fn renders_foldable_callout_without_blank_line() {
        let html = RushdownMarkdownRenderer::new()
            .render_html(
                r#"
> [!faq]- Are callouts foldable?
> Yes! In a foldable callout, the contents are hidden when the callout is collapsed.
"#,
            )
            .expect("markdown should render");

        assert!(html.contains(r#"data-callout="question""#), "{html}");
        assert!(
            html.contains(r#"class="callout is-collapsible is-collapsed""#),
            "{html}"
        );
        assert!(html.contains("Are callouts foldable?"), "{html}");
        assert!(
            html.contains(
                "<p>Yes! In a foldable callout, the contents are hidden when the callout is collapsed.</p>"
            ),
            "{html}"
        );
        assert!(!html.contains("[!faq]"), "{html}");
    }

    #[test]
    fn renders_nested_callouts() {
        let html = RushdownMarkdownRenderer::new()
            .render_html(
                r#"
> [!question] Can callouts be nested?
> > [!todo] Yes!, they can.
> > > [!example] You can even use multiple layers of nesting.
"#,
            )
            .expect("markdown should render");

        assert!(html.contains(r#"data-callout="question""#), "{html}");
        assert!(html.contains(r#"data-callout="todo""#), "{html}");
        assert!(html.contains(r#"data-callout="example""#), "{html}");
        assert!(html.contains("<p>Can callouts be nested?</p>"), "{html}");
        assert!(html.contains("<p>Yes!, they can.</p>"), "{html}");
        assert!(
            html.contains("<p>You can even use multiple layers of nesting.</p>"),
            "{html}"
        );
    }

    #[test]
    fn renders_link_and_image_attributes() {
        let html = RushdownMarkdownRenderer::new()
            .render_html(
                r#"
[aaa](https://example.com/aaa){.myclass #example data-kind="link"}

![alt text](/media/images/example){.image-class data-size="large"}
"#,
            )
            .expect("markdown should render");

        assert!(
            html.contains(
                r#"<a href="https://example.com/aaa" class="external myclass" id="example" data-kind="link">aaa<svg class="external-icon""#
            ),
            "{html}",
        );
        assert!(
            html.contains(
                r#"<img src="/media/images/example" alt="alt text" class="image-class" data-size="large">"#
            ),
            "{html}",
        );
    }

    #[test]
    fn classifies_internal_and_external_links() {
        let html = RushdownMarkdownRenderer::new()
            .render_html(
                r#"
[notes](./notes/) [root](/osib/) [home](.) [external](https://example.com/path)
"#,
            )
            .expect("markdown should render");

        assert!(
            html.contains(
                r#"<a href="./notes/" class="internal alias" data-slug="notes/index">notes</a>"#
            ),
            "{html}",
        );
        assert!(
            html.contains(
                r#"<a href="/osib/" class="internal alias" data-slug="osib/index">root</a>"#
            ),
            "{html}",
        );
        assert!(
            html.contains(r#"<a href="." class="internal alias" data-slug="index">home</a>"#),
            "{html}",
        );
        assert!(
            html.contains(
                r#"<a href="https://example.com/path" class="external">external<svg class="external-icon""#
            ),
            "{html}",
        );
    }

    #[test]
    fn does_not_classify_image_badge_links_as_external() {
        let html = RushdownMarkdownRenderer::new()
            .render_html(
                r#"
[![stars](https://img.shields.io/github/stars/example/repo)](https://github.com/example/repo)
"#,
            )
            .expect("markdown should render");

        assert!(
            html.contains(r#"<a href="https://github.com/example/repo"><img"#),
            "{html}",
        );
        assert!(!html.contains(r#"class="external""#), "{html}");
        assert!(!html.contains(r#"external-icon"#), "{html}");
    }
}
