use rushdown::{
    new_markdown_to_html_string,
    parser::{self, ParserExtension},
    renderer::html::{self, RendererExtension},
};
use rushdown_diagram::{
    DiagramHtmlRendererOptions, DiagramParserOptions, diagram_html_renderer_extension,
    diagram_parser_extension,
};
use rushdown_fenced_div::{
    FencedDivHtmlRendererOptions, fenced_div_html_renderer_extension, fenced_div_parser_extension,
};
use rushdown_footnote::{
    FootnoteHtmlRendererOptions, footnote_html_renderer_extension, footnote_parser_extension,
};
use rushdown_highlighting::{
    HighlightingHtmlRendererOptions, HighlightingMode, highlighting_html_renderer_extension,
};
use rushdown_link_attribute::link_attribute_parser_extension;
use yaml_rust::YamlLoader;

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
            .and(fenced_div_parser_extension())
            .and(footnote_parser_extension())
            .and(diagram_parser_extension(DiagramParserOptions::default()))
            .and(link_attribute_parser_extension())
            .and(parser::gfm_table())
            .and(parser::gfm_task_list_item())
    }

    fn renderer_extensions(&self) -> impl RendererExtension<'_> {
        fenced_div_html_renderer_extension(FencedDivHtmlRendererOptions::default())
            .and(footnote_html_renderer_extension(
                FootnoteHtmlRendererOptions::default(),
            ))
            .and(diagram_html_renderer_extension(
                DiagramHtmlRendererOptions::default(),
            ))
            .and(highlighting_html_renderer_extension(
                HighlightingHtmlRendererOptions {
                    mode: HighlightingMode::Attribute,
                    ..HighlightingHtmlRendererOptions::default()
                },
            ))
    }
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
    fn renders_fenced_divs() {
        let html = RushdownMarkdownRenderer::new()
            .render_html(
                r#"
::: {.note #tip data-kind="callout"} :::
inside
::::::::::::::::::::::::::::::::::::::::
"#,
            )
            .expect("markdown should render");

        assert_eq!(
            html.trim(),
            r#"<div class="note" id="tip" data-kind="callout"><p>inside</p></div>"#
        );
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
                r#"<a href="https://example.com/aaa" class="myclass" id="example" data-kind="link">aaa</a>"#
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
}
