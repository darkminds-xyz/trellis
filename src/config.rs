use std::{fs, path::Path};

use anyhow::Context;
use yaml_rust::{Yaml, YamlLoader};

#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub database_url: Option<String>,
    pub admin: AdminConfig,
    pub typography: TypographyConfig,
    pub site: SiteConfig,
    pub server: ServerConfig,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 47055,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AdminConfig {
    pub username: Option<String>,
    pub password: Option<String>,
    pub secure_cookies: bool,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            username: None,
            password: None,
            secure_cookies: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TypographyConfig {
    pub body_font: Option<String>,
    pub heading_font: Option<String>,
    pub mono_font: Option<String>,
    pub title_font: Option<String>,
    pub google_fonts_href: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SiteConfig {
    pub name: String,
    pub tagline: Option<String>,
    pub code: CodeThemeConfig,
    pub colors: SiteColorsConfig,
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            name: "Trellis".to_string(),
            tagline: None,
            code: CodeThemeConfig::default(),
            colors: SiteColorsConfig::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodeThemeConfig {
    pub light: String,
    pub dark: String,
}

impl Default for CodeThemeConfig {
    fn default() -> Self {
        Self {
            light: "Github_Light".to_string(),
            dark: "Github_Dark".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SiteColorsConfig {
    pub light: ThemeColorsConfig,
    pub dark: ThemeColorsConfig,
}

impl Default for SiteColorsConfig {
    fn default() -> Self {
        Self {
            light: ThemeColorsConfig {
                light: "#ffffffd1".to_string(),
                lightgray: "#e5e5e5".to_string(),
                gray: "#b8b8b8".to_string(),
                darkgray: "#3b3a3a".to_string(),
                dark: "#06070b".to_string(),
                secondary: "#008066".to_string(),
                tertiary: "#005042e6".to_string(),
                highlight: "#8f9fa914".to_string(),
            },
            dark: ThemeColorsConfig {
                light: "#06070b".to_string(),
                lightgray: "#141e22".to_string(),
                gray: "#6b6b6b".to_string(),
                darkgray: "#d4d4d4".to_string(),
                dark: "#ffffffd1".to_string(),
                secondary: "#008066".to_string(),
                tertiary: "#0fd392b3".to_string(),
                highlight: "#191d1d96".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThemeColorsConfig {
    pub light: String,
    pub lightgray: String,
    pub gray: String,
    pub darkgray: String,
    pub dark: String,
    pub secondary: String,
    pub tertiary: String,
    pub highlight: String,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        Self::load_from_path("config.yml")
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }

        let source = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Self::from_yaml_str(&source).with_context(|| format!("failed to parse {}", path.display()))
    }

    fn from_yaml_str(source: &str) -> anyhow::Result<Self> {
        let docs = YamlLoader::load_from_str(source)?;
        let Some(root) = docs.first() else {
            return Ok(Self::default());
        };

        let default_site = SiteConfig::default();
        let default_server = ServerConfig::default();

        Ok(Self {
            server: ServerConfig {
                host: string(root, &["server", "host"]).unwrap_or(default_server.host),
                port: number_u16(root, &["server", "port"]).unwrap_or(default_server.port),
            },
            database_url: string(root, &["database_url"])
                .or_else(|| string(root, &["database", "url"])),
            admin: AdminConfig {
                username: string(root, &["admin", "username"]),
                password: string(root, &["admin", "password"]),
                secure_cookies: boolean(root, &["admin", "secure_cookies"]).unwrap_or(true),
            },
            typography: TypographyConfig {
                body_font: string(root, &["typography", "body_font"])
                    .or_else(|| string(root, &["typography", "body"])),
                heading_font: string(root, &["typography", "heading_font"])
                    .or_else(|| string(root, &["typography", "heading"])),
                mono_font: string(root, &["typography", "mono_font"])
                    .or_else(|| string(root, &["typography", "mono"])),
                title_font: string(root, &["typography", "title_font"])
                    .or_else(|| string(root, &["typography", "title"])),
                google_fonts_href: string(root, &["typography", "google_fonts_href"])
                    .or_else(|| string(root, &["typography", "href"])),
            },
            site: SiteConfig {
                name: string(root, &["site", "name"]).unwrap_or(default_site.name),
                tagline: string(root, &["site", "tagline"]),
                code: CodeThemeConfig {
                    light: string(root, &["site", "code", "light"])
                        .unwrap_or(default_site.code.light),
                    dark: string(root, &["site", "code", "dark"]).unwrap_or(default_site.code.dark),
                },
                colors: SiteColorsConfig {
                    light: theme_colors(
                        root,
                        &["site", "colors", "lightmode"],
                        default_site.colors.light,
                    ),
                    dark: theme_colors(
                        root,
                        &["site", "colors", "darkmode"],
                        default_site.colors.dark,
                    ),
                },
            },
        })
    }
}

fn theme_colors(root: &Yaml, path: &[&str], defaults: ThemeColorsConfig) -> ThemeColorsConfig {
    ThemeColorsConfig {
        light: string(root, &[path, &["light"]].concat()).unwrap_or(defaults.light),
        lightgray: string(root, &[path, &["lightgray"]].concat()).unwrap_or(defaults.lightgray),
        gray: string(root, &[path, &["gray"]].concat()).unwrap_or(defaults.gray),
        darkgray: string(root, &[path, &["darkgray"]].concat()).unwrap_or(defaults.darkgray),
        dark: string(root, &[path, &["dark"]].concat()).unwrap_or(defaults.dark),
        secondary: string(root, &[path, &["secondary"]].concat()).unwrap_or(defaults.secondary),
        tertiary: string(root, &[path, &["tertiary"]].concat()).unwrap_or(defaults.tertiary),
        highlight: string(root, &[path, &["highlight"]].concat()).unwrap_or(defaults.highlight),
    }
}

fn string(root: &Yaml, path: &[&str]) -> Option<String> {
    let value = path.iter().try_fold(root, |node, key| {
        let Yaml::Hash(hash) = node else {
            return None;
        };

        hash.get(&Yaml::String((*key).to_string()))
    })?;

    match value {
        Yaml::String(value) => non_empty(value),
        Yaml::Integer(value) => non_empty(&value.to_string()),
        Yaml::Real(value) => non_empty(value),
        Yaml::Boolean(value) => non_empty(&value.to_string()),
        _ => None,
    }
}

fn number_u16(root: &Yaml, path: &[&str]) -> Option<u16> {
    let value = path.iter().try_fold(root, |node, key| {
        let Yaml::Hash(hash) = node else {
            return None;
        };

        hash.get(&Yaml::String((*key).to_string()))
    })?;

    match value {
        Yaml::Integer(value) => Some(*value as u16),
        _ => None,
    }
}

fn boolean(root: &Yaml, path: &[&str]) -> Option<bool> {
    let value = path.iter().try_fold(root, |node, key| {
        let Yaml::Hash(hash) = node else {
            return None;
        };

        hash.get(&Yaml::String((*key).to_string()))
    })?;

    match value {
        Yaml::Boolean(value) => Some(*value),
        Yaml::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn reads_nested_config_yml_values() {
        let config = AppConfig::from_yaml_str(
            r##"
database:
  url: data/trellis.db
admin:
  username: admin
  password: secret
  secure_cookies: false
typography:
  body_font: Inter
  heading_font: Lora
  mono_font: "JetBrains Mono"
  title_font: Lora
  google_fonts_href: https://fonts.example.test/css
site:
  name: My Notes
  tagline: Working notes
  code:
    light: Github_Light.tmTheme
    dark: Github_Dark.tmTheme
  colors:
    lightmode:
      light: "#fff"
      secondary: "var(--custom-secondary)"
    darkmode:
      dark: "#eee"
      highlight: "rgb(1 2 3 / 40%)"
"##,
        )
        .expect("config should parse");

        assert_eq!(config.database_url.as_deref(), Some("data/trellis.db"));
        assert_eq!(config.admin.username.as_deref(), Some("admin"));
        assert_eq!(config.admin.password.as_deref(), Some("secret"));
        assert!(!config.admin.secure_cookies);
        assert_eq!(config.typography.body_font.as_deref(), Some("Inter"));
        assert_eq!(config.typography.heading_font.as_deref(), Some("Lora"));
        assert_eq!(
            config.typography.mono_font.as_deref(),
            Some("JetBrains Mono")
        );
        assert_eq!(config.site.name, "My Notes");
        assert_eq!(config.site.tagline.as_deref(), Some("Working notes"));
        assert_eq!(config.site.code.light, "Github_Light.tmTheme");
        assert_eq!(config.site.code.dark, "Github_Dark.tmTheme");
        assert_eq!(config.site.colors.light.light, "#fff");
        assert_eq!(
            config.site.colors.light.secondary,
            "var(--custom-secondary)"
        );
        assert_eq!(config.site.colors.light.dark, "#06070b");
        assert_eq!(config.site.colors.dark.dark, "#eee");
        assert_eq!(config.site.colors.dark.highlight, "rgb(1 2 3 / 40%)");
        assert_eq!(config.site.colors.dark.light, "#06070b");
    }

    #[test]
    fn reads_short_typography_config_keys() {
        let config = AppConfig::from_yaml_str(
            r#"
typography:
  href: https://fonts.example.test/short.css
  body: Lato
  heading: Questrial
  mono: "JetBrains Mono"
  title: "Zilla Slab Highlight"
"#,
        )
        .expect("config should parse");

        assert_eq!(config.typography.body_font.as_deref(), Some("Lato"));
        assert_eq!(config.typography.heading_font.as_deref(), Some("Questrial"));
        assert_eq!(
            config.typography.mono_font.as_deref(),
            Some("JetBrains Mono")
        );
        assert_eq!(
            config.typography.title_font.as_deref(),
            Some("Zilla Slab Highlight")
        );
        assert_eq!(
            config.typography.google_fonts_href.as_deref(),
            Some("https://fonts.example.test/short.css")
        );
    }
}
