use config::{Config, ConfigError, File};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Settings {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_empty")]
    pub path: String,
    #[serde(default = "default_empty")]
    pub token: String,
    pub api_id: i32,
    pub api_hash: String,
    #[serde(default = "default_empty")]
    pub phone: String,
    #[serde(default = "default_empty")]
    pub password: String,
    #[serde(default = "default_empty")]
    pub bot_token: String,
    #[serde(default = "default_empty")]
    pub proxy: String,
}

fn default_bind() -> String {
    "127.0.0.1:5140".to_string()
}

fn default_empty() -> String {
    String::new()
}

impl Settings {
    pub fn read() -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(File::with_name("config"))
            .build()?;
        config.try_deserialize()
    }
}
