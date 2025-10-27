use anyhow::{Context, Result, anyhow};
use dotenvy::dotenv;
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use url::Url;

pub const KEYRING_SERVICE: &str = "adpt-api-key";
pub const KEYRING_USER: &str = "Adaptive";

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct ConfigFile {
    pub default_use_case: Option<String>,
    pub adaptive_base_url: Option<Url>,
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

    let mut adaptive_base_url = override_config
        .adaptive_base_url
        .or(base.adaptive_base_url)
        .ok_or(anyhow!("No adaptive base URL provided"))?;

    adaptive_base_url = adaptive_base_url
        .join("/api/graphql")
        .context("Failed to append /api/graphql to base URL")?;

    let adaptive_api_key = if let Some(api_key) = override_config.adaptive_api_key {
        api_key
    } else {
        let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)?;
        let api_key = entry.get_secret().context(
            "API key not specified via environment variable nor present in OS keyring.\n\
            Use `adpt set-api-key <your-key>` to set it.",
        )?;
        String::from_utf8(api_key)?
    };

    Ok(Config {
        default_use_case,
        adaptive_base_url,
        adaptive_api_key,
    })
}

fn get_config_file_path() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let base_dirs =
            directories::BaseDirs::new().ok_or(anyhow!("Unable to determine home directory"))?;
        Ok(base_dirs.home_dir().join(".adpt").join("config.toml"))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let project_dirs = directories::ProjectDirs::from("com", "adaptive-ml", "adpt")
            .ok_or(anyhow!("Unable to determine home directory"))?;
        Ok(project_dirs.config_dir().join("config.toml"))
    }
}

pub fn read_config() -> Result<Config> {
    let _ = dotenv();
    let env_config = envy::from_env::<ConfigEnv>().unwrap_or_default();

    let config_file = get_config_file_path()?;
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

pub fn write_config(config: ConfigFile) -> Result<()> {
    let config_file_path = get_config_file_path()?;

    // Create parent directories if they don't exist
    if let Some(parent) = config_file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let toml_string = toml::to_string_pretty(&config)?;
    fs::write(&config_file_path, toml_string)?;

    println!("\nConfiguration saved to {}", config_file_path.display());
    Ok(())
}
