use rushdown::{
    new_markdown_to_html_string,
    parser::{self, ParserExtension},
    renderer::html,
};
use rushdown_highlighting::{
    HighlightingHtmlRendererOptions, HighlightingMode, highlighting_html_renderer_extension,
};

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
            .and(parser::gfm_table())
            .and(parser::gfm_task_list_item())
    }
}

impl MarkdownHtmlRenderer for RushdownMarkdownRenderer {
    fn render_html(&self, markdown: &str) -> rushdown::Result<String> {
        let render = new_markdown_to_html_string(
            self.parser_options.clone(),
            self.renderer_options.clone(),
            self.parser_extensions(),
            highlighting_html_renderer_extension(HighlightingHtmlRendererOptions {
                mode: HighlightingMode::Attribute,
                ..HighlightingHtmlRendererOptions::default()
            }),
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
}
