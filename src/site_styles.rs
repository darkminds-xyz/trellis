use crate::config::{SiteColorsConfig, ThemeColorsConfig};

pub fn site_color_css(colors: &SiteColorsConfig) -> String {
    format!(
        "{light_css}\n{dark_css}",
        light_css = theme_color_block(":root", &colors.light),
        dark_css = theme_color_block(":root[saved-theme=\"dark\"]", &colors.dark),
    )
}

fn theme_color_block(selector: &str, colors: &ThemeColorsConfig) -> String {
    format!(
        r#"{selector} {{
  --light: {light};
  --lightgray: {lightgray};
  --gray: {gray};
  --darkgray: {darkgray};
  --dark: {dark};
  --secondary: {secondary};
  --tertiary: {tertiary};
  --highlight: {highlight};
}}
"#,
        selector = selector,
        light = colors.light,
        lightgray = colors.lightgray,
        gray = colors.gray,
        darkgray = colors.darkgray,
        dark = colors.dark,
        secondary = colors.secondary,
        tertiary = colors.tertiary,
        highlight = colors.highlight,
    )
}

#[cfg(test)]
mod tests {
    use crate::config::SiteColorsConfig;

    use super::site_color_css;

    #[test]
    fn renders_light_and_dark_color_variables() {
        let css = site_color_css(&SiteColorsConfig::default());

        assert!(css.contains(":root {"));
        assert!(css.contains("--light: #ffffffd1;"));
        assert!(css.contains(":root[saved-theme=\"dark\"] {"));
        assert!(css.contains("--light: #06070b;"));
    }
}
