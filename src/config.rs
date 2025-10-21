use anyhow::{Context, Result, anyhow};
use dotenvy::dotenv;
use keyring::Entry;
use serde::Deserialize;
use std::fs;
use url::Url;

pub const KEYRING_SERVICE: &str = "adpt-api-key";
pub const KEYRING_USER: &str = "Adaptive";

#[derive(Debug, Deserialize, Default)]
struct ConfigFile {
    default_use_case: Option<String>,
    adaptive_base_url: Option<Url>,
}

#[derive(Debug, Deserialize, Default)]
struct ConfigEnv {
    default_use_case: Option<String>,
    adaptive_base_url: Option<Url>,
    adaptive_api_key: Option<String>,
}

pub struct Config {
    pub default_use_case: Option<String>,
    pub adaptive_base_url: Url,
    pub adaptive_api_key: String,
}

fn merge_config(base: ConfigFile, override_config: ConfigEnv) -> Result<Config> {
    let default_use_case = override_config.default_use_case.or(base.default_use_case);

    let adaptive_base_url = override_config
        .adaptive_base_url
        .or(base.adaptive_base_url)
        .ok_or(anyhow!("No adaptive base URL provided"))?;

    let adaptive_api_key = if let Some(api_key) = override_config.adaptive_api_key {
        api_key
    } else {
        let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)?;
        let api_key = entry
            .get_secret()
            .context("API key not specified via environment variable nor present in OS keyring")?;
        String::from_utf8(api_key)?
    };

    Ok(Config {
        default_use_case,
        adaptive_base_url,
        adaptive_api_key,
    })
}

pub fn read_config() -> Result<Config> {
    let _ = dotenv();
    let env_config = envy::from_env::<ConfigEnv>().unwrap_or_default();

    let project_dirs = directories::ProjectDirs::from("com", "adaptive-ml", "adpt")
        .ok_or(anyhow!("Unable to determine home directory"))?;
    let config_file = project_dirs.config_dir().join("config.toml");
    let file_config = if let Ok(config) = fs::read_to_string(config_file) {
        toml::from_str(&config)?
    } else {
        ConfigFile::default()
    };

    merge_config(file_config, env_config)
}

pub fn set_api_key_keyring(api_key: String) -> Result<()> {
    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)?;
    entry.set_secret(api_key.as_bytes())?;
    println!("API key set for use with adpt");
    Ok(())
}
