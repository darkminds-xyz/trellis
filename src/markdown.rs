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

pub trait MarkdownHtmlRenderer {
    fn render_html(&self, markdown: &str) -> rushdown::Result<String>;
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
    use super::{MarkdownHtmlRenderer, RushdownMarkdownRenderer};

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
