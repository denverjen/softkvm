use anyhow::Result;
use serde::{Deserialize, Serialize};
use softkvm_protocol::DEFAULT_PORT;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub client: ClientConfig,
    pub layout: LayoutConfig,
    #[serde(default)]
    pub clipboard: ClipboardConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    pub listen: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientConfig {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LayoutConfig {
    #[serde(default = "default_layout")]
    pub position: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClipboardConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_size")]
    pub max_size: usize,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_size: 1048576,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub tls: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self { tls: false }
    }
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

fn default_layout() -> String {
    "left-right".to_string()
}

fn default_true() -> bool {
    true
}

fn default_max_size() -> usize {
    1048576
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn load_or_default(path: &str) -> Self {
        Self::load(path).unwrap_or_else(|_| Self::default_config())
    }

    fn default_config() -> Self {
        Self {
            server: ServerConfig {
                listen: "0.0.0.0".to_string(),
                port: DEFAULT_PORT,
            },
            client: ClientConfig {
                host: "127.0.0.1".to_string(),
                port: DEFAULT_PORT,
            },
            layout: LayoutConfig {
                position: "left-right".to_string(),
            },
            clipboard: ClipboardConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}
