use std::path::Path;

use log::warn;
use syntect::highlighting::ThemeSet;

use crate::config::CodeThemeConfig;

const DEFAULT_LIGHT_THEME: &str = "Github_Light";
const DEFAULT_DARK_THEME: &str = "Github_Dark";
const THEME_DIR: &str = "src/themes";

pub fn syntax_theme_css(config: &CodeThemeConfig) -> String {
    let theme_set = load_theme_set(THEME_DIR);
    let light_theme = configured_theme(&theme_set, &config.light, DEFAULT_LIGHT_THEME, "light");
    let dark_theme = configured_theme(&theme_set, &config.dark, DEFAULT_DARK_THEME, "dark");

    let mut css = String::new();
    if let Some(light_css) = rushdown_highlighting::generate_css(&light_theme, Some(&theme_set)) {
        css.push_str(&scope_css(":root", &light_css));
    } else {
        warn!("failed to generate CSS for light code theme '{light_theme}'");
    }

    if let Some(dark_css) = rushdown_highlighting::generate_css(&dark_theme, Some(&theme_set)) {
        css.push_str(&scope_css(":root[saved-theme=\"dark\"]", &dark_css));
    } else {
        warn!("failed to generate CSS for dark code theme '{dark_theme}'");
    }

    css
}

fn load_theme_set(theme_dir: impl AsRef<Path>) -> ThemeSet {
    let theme_dir = theme_dir.as_ref();
    let mut theme_set = ThemeSet::new();
    let paths = match ThemeSet::discover_theme_paths(theme_dir) {
        Ok(paths) => paths,
        Err(err) => {
            warn!(
                "failed to discover code themes in '{}': {err}",
                theme_dir.display()
            );
            return theme_set;
        }
    };

    for path in paths {
        let Some(theme_name) = path.file_stem().and_then(|name| name.to_str()) else {
            warn!(
                "skipping code theme with invalid file name '{}'",
                path.display()
            );
            continue;
        };

        match ThemeSet::get_theme(&path) {
            Ok(theme) => {
                theme_set.themes.insert(theme_name.to_string(), theme);
            }
            Err(err) => {
                warn!("failed to load code theme '{}': {err}", path.display());
            }
        }
    }

    theme_set
}

fn configured_theme(
    theme_set: &ThemeSet,
    configured_name: &str,
    fallback_name: &str,
    color_scheme: &str,
) -> String {
    let theme_name = normalize_theme_name(configured_name);
    if theme_set.themes.contains_key(&theme_name) {
        return theme_name;
    }

    warn!(
        "configured {color_scheme} code theme '{}' was not found in {}; falling back to '{fallback_name}'",
        configured_name, THEME_DIR
    );

    if theme_set.themes.contains_key(fallback_name) {
        fallback_name.to_string()
    } else {
        warn!("fallback {color_scheme} code theme '{fallback_name}' was not found in {THEME_DIR}");
        theme_name
    }
}

fn normalize_theme_name(name: &str) -> String {
    name.trim()
        .strip_suffix(".tmTheme")
        .unwrap_or_else(|| name.trim())
        .to_string()
}

fn scope_css(scope: &str, css: &str) -> String {
    css.lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.ends_with('{') && !trimmed.starts_with('@') {
                format!("{scope} {line}\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{normalize_theme_name, scope_css};

    #[test]
    fn normalizes_theme_file_names_to_theme_keys() {
        assert_eq!(normalize_theme_name("Github_Light.tmTheme"), "Github_Light");
        assert_eq!(normalize_theme_name("Github_Dark"), "Github_Dark");
    }

    #[test]
    fn scopes_generated_syntect_selectors() {
        let scoped = scope_css(
            ":root[saved-theme=\"dark\"]",
            ".code {\n color: #fff;\n}\n\n.keyword {\n color: #f00;\n}\n",
        );

        assert!(scoped.contains(":root[saved-theme=\"dark\"] .code {"));
        assert!(scoped.contains(":root[saved-theme=\"dark\"] .keyword {"));
        assert!(scoped.contains(" color: #fff;"));
    }
}
