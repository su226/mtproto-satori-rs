use config::{Config, ConfigError, File};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct MergeMediaGroup {
    #[serde(default = "default_merge_media_group_receive")]
    pub receive: u64,
}

fn default_merge_media_group_receive() -> u64 {
    100
}

fn default_merge_media_group() -> MergeMediaGroup {
    MergeMediaGroup { receive: 100 }
}

#[derive(Deserialize)]
pub struct Settings {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_empty")]
    pub path: String,
    #[serde(default = "default_empty")]
    pub token: String,
    #[serde(default = "default_json_size_limit")]
    pub json_size_limit: usize,
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
    #[serde(default = "default_merge_media_group")]
    pub merge_media_group: MergeMediaGroup,
}

fn default_bind() -> String {
    "127.0.0.1:5140".to_string()
}

fn default_json_size_limit() -> usize {
    10 * 1024 * 1024
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
