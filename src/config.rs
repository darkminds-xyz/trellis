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
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            name: "Trellis".to_string(),
            tagline: None,
        }
    }
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
                .or_else(|| string(root, &["database", "url"]))
                .or_else(|| string(root, &["DATABASE_URL"])),
            admin: AdminConfig {
                username: string(root, &["admin", "username"]),
                password: string(root, &["admin", "password"]),
                secure_cookies: boolean(root, &["admin", "secure_cookies"]).unwrap_or(true),
            },
            typography: TypographyConfig {
                body_font: string(root, &["typography", "body_font"]),
                heading_font: string(root, &["typography", "heading_font"]),
                mono_font: string(root, &["typography", "mono_font"]),
                title_font: string(root, &["typography", "title_font"]),
                google_fonts_href: string(root, &["typography", "google_fonts_href"]),
            },
            site: SiteConfig {
                name: string(root, &["site", "name"]).unwrap_or(default_site.name),
                tagline: string(root, &["site", "tagline"]),
            },
        })
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
            r#"
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
"#,
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
    }
}
