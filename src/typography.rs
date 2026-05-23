use crate::config::TypographyConfig;

#[derive(Debug, Clone)]
pub struct Typography {
    pub fonts_href: String,
    pub font_css: String,
}

impl Typography {
    pub fn from_config(config: &TypographyConfig) -> Self {
        let body_font = config
            .body_font
            .as_deref()
            .unwrap_or("Lato")
            .trim()
            .to_string();
        let heading_font = config
            .heading_font
            .as_deref()
            .unwrap_or(&body_font)
            .trim()
            .to_string();
        let mono_font = config
            .mono_font
            .as_deref()
            .unwrap_or("monospace")
            .trim()
            .to_string();
        let title_font = config
            .title_font
            .as_deref()
            .unwrap_or(&heading_font)
            .trim()
            .to_string();
        let fonts_href = config.google_fonts_href.clone().unwrap_or_default();

        let font_css = format!(
            r#"
:root {{
  --bodyFont: {};
  --headerFont: {};
  --codeFont: {};
  --titleFont: {};
}}
.page > #trellis-body .sidebar.left > .page-header .page-title {{
  font-family: var(--titleFont);
}}
.page-content {{
  font-family: var(--bodyFont);
}}
.page-content :is(h1, h2, h3, h4, h5, h6) {{
  font-family: var(--headerFont);
}}
.page-content :is(code, pre, kbd, samp), #trellis-editor .cm-fenced-code-line {{
  font-family: var(--codeFont);
}}
#trellis-editor, #trellis-editor .cm-editor, #trellis-editor .cm-content {{
  font-family: var(--bodyFont);
}}
#trellis-editor :is(h1, h2, h3, h4, h5, h6), #trellis-editor .cm-header {{
  font-family: var(--headerFont);
}}
"#,
            font_stack(&body_font, &["system-ui", "-apple-system", "sans-serif"]),
            font_stack(&heading_font, &["var(--bodyFont)"]),
            font_stack(&mono_font, &["monospace"]),
            font_stack(&title_font, &["var(--headerFont)"]),
        );

        Self {
            fonts_href,
            font_css,
        }
    }
}

fn font_stack(primary: &str, fallback: &[&str]) -> String {
    let mut stack = vec![font_family(primary)];
    stack.extend(fallback.iter().map(|font| (*font).to_string()));
    stack.join(", ")
}

fn font_family(value: &str) -> String {
    if is_css_keyword_or_var(value) {
        return value.to_string();
    }

    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn is_css_keyword_or_var(value: &str) -> bool {
    value.starts_with("var(")
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "serif"
                | "sans-serif"
                | "monospace"
                | "cursive"
                | "fantasy"
                | "system-ui"
                | "ui-serif"
                | "ui-sans-serif"
                | "ui-monospace"
                | "emoji"
                | "math"
                | "fangsong"
        )
}
