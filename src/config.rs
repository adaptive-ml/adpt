use anyhow::{Context, Result, anyhow, bail};
use dotenvy::dotenv;
use keyring::Entry;
use serde::{Deserialize, Serialize};
use slug::slugify;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use url::Url;

/// Normalize a user-provided deployment name into a stable slug.
pub fn normalize_name(name: &str) -> String {
    slugify(name)
}

pub const KEYRING_SERVICE: &str = "adpt-api-key";
const LEGACY_KEYRING_USER: &str = "Adaptive";

fn keyring_user(deployment: &str) -> String {
    format!("Adaptive:{deployment}")
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeploymentConfig {
    pub adaptive_base_url: Url,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_project: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct ConfigFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_deployment: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub deployments: BTreeMap<String, DeploymentConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct ConfigFileRaw {
    #[serde(default)]
    active_deployment: Option<String>,
    #[serde(default)]
    deployments: BTreeMap<String, DeploymentConfig>,
    // Legacy flat fields, present in pre-FE-26 configs.
    #[serde(default)]
    adaptive_base_url: Option<Url>,
    #[serde(default)]
    default_project: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ConfigEnv {
    default_project: Option<String>,
    adaptive_base_url: Option<Url>,
    adaptive_api_key: Option<String>,
}

pub struct Config {
    pub deployment_name: String,
    pub default_project: Option<String>,
    pub adaptive_base_url: Url,
    pub adaptive_api_key: String,
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

/// Read the config file from disk and apply legacy migration in-memory.
/// If a legacy flat config is detected, this also persists the migrated form
/// and moves the keyring entry from `Adaptive` to `Adaptive:default`.
pub fn read_config_file() -> Result<ConfigFile> {
    let path = get_config_file_path()?;
    let Ok(raw_str) = fs::read_to_string(&path) else {
        return Ok(ConfigFile::default());
    };
    let raw: ConfigFileRaw = toml::from_str(&raw_str)?;

    let needs_migration = raw.deployments.is_empty() && raw.adaptive_base_url.is_some();

    let mut file = ConfigFile {
        active_deployment: raw.active_deployment,
        deployments: raw.deployments,
    };

    if needs_migration && let Some(url) = raw.adaptive_base_url {
        file.deployments.insert(
            "default".to_string(),
            DeploymentConfig {
                adaptive_base_url: url,
                default_project: raw.default_project,
            },
        );
        file.active_deployment = Some("default".to_string());
        write_config_file(&file)?;
        let _ = migrate_legacy_keyring();
    }

    Ok(file)
}

pub fn write_config_file(config: &ConfigFile) -> Result<()> {
    let path = get_config_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let toml_string = toml::to_string_pretty(config)?;
    fs::write(&path, toml_string)?;
    Ok(())
}

fn migrate_legacy_keyring() -> Result<()> {
    let legacy = Entry::new(KEYRING_SERVICE, LEGACY_KEYRING_USER)?;
    let secret = legacy.get_secret()?;
    let new = Entry::new(KEYRING_SERVICE, &keyring_user("default"))?;
    new.set_secret(&secret)?;
    let _ = legacy.delete_credential();
    Ok(())
}

/// Resolve the active deployment.
///
/// Order of precedence:
///   1. explicit `--deployment` override
///   2. `$ADPT_DEPLOYMENT` env var (pins a shell, kubie-style)
///   3. `active_deployment` field in the config file
///   4. sole configured deployment if exactly one exists
fn resolve_active_name(file: &ConfigFile, override_name: Option<&str>) -> Result<String> {
    if let Some(name) = override_name {
        let name = normalize_name(name);
        if !file.deployments.contains_key(&name) {
            bail!("Deployment `{name}` is not configured. Run `adpt deployment setup {name}`.");
        }
        return Ok(name);
    }
    if let Ok(env_name) = std::env::var("ADPT_DEPLOYMENT")
        && !env_name.is_empty()
    {
        let name = normalize_name(&env_name);
        if !file.deployments.contains_key(&name) {
            bail!(
                "Deployment `{name}` (from $ADPT_DEPLOYMENT) is not configured. \
                 Run `adpt deployment setup {name}` or unset the env var."
            );
        }
        return Ok(name);
    }
    if let Some(name) = &file.active_deployment
        && file.deployments.contains_key(name)
    {
        return Ok(name.clone());
    }
    if file.deployments.len() == 1 {
        return Ok(file.deployments.keys().next().unwrap().clone());
    }
    if file.deployments.is_empty() {
        bail!("No deployment configured. Run `adpt deployment setup <name>`.");
    }
    bail!(
        "No active deployment selected. Run `adpt deployment use <name>` (configured: {}).",
        file.deployments
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    );
}

pub fn read_config(deployment_override: Option<&str>) -> Result<Config> {
    let _ = dotenv();
    let env_config = envy::from_env::<ConfigEnv>().unwrap_or_default();
    let file = read_config_file()?;
    let name = resolve_active_name(&file, deployment_override)?;
    let base = file
        .deployments
        .get(&name)
        .cloned()
        .ok_or_else(|| anyhow!("Deployment `{name}` not found"))?;

    let default_project = env_config.default_project.or(base.default_project);

    let mut adaptive_base_url = env_config
        .adaptive_base_url
        .unwrap_or(base.adaptive_base_url);
    adaptive_base_url = adaptive_base_url
        .join("/api/")
        .context("Failed to append /api to base URL")?;

    let adaptive_api_key = if let Some(api_key) = env_config.adaptive_api_key {
        api_key
    } else {
        get_api_key(&name)?.ok_or_else(|| {
            anyhow!(
                "API key for deployment `{name}` not found in OS keyring.\n\
                 Run `adpt deployment setup {name}` to set it."
            )
        })?
    };

    Ok(Config {
        deployment_name: name,
        default_project,
        adaptive_base_url,
        adaptive_api_key,
    })
}

pub fn get_api_key(deployment: &str) -> Result<Option<String>> {
    let entry = Entry::new(KEYRING_SERVICE, &keyring_user(deployment))?;
    match entry.get_secret() {
        Ok(bytes) => Ok(Some(String::from_utf8(bytes)?)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn set_api_key(deployment: &str, api_key: &str) -> Result<()> {
    let entry = Entry::new(KEYRING_SERVICE, &keyring_user(deployment))?;
    entry.set_secret(api_key.as_bytes())?;
    Ok(())
}

pub fn delete_api_key(deployment: &str) -> Result<()> {
    let entry = Entry::new(KEYRING_SERVICE, &keyring_user(deployment))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub fn upsert_deployment(name: &str, deployment: DeploymentConfig) -> Result<()> {
    let name = normalize_name(name);
    let mut file = read_config_file()?;
    let is_new = !file.deployments.contains_key(&name);
    file.deployments.insert(name.clone(), deployment);
    if is_new {
        file.active_deployment = Some(name);
    }
    write_config_file(&file)
}

pub fn set_active(name: &str) -> Result<()> {
    let name = normalize_name(name);
    let mut file = read_config_file()?;
    if !file.deployments.contains_key(&name) {
        bail!("Deployment `{name}` is not configured.");
    }
    file.active_deployment = Some(name);
    write_config_file(&file)
}

pub fn remove_deployment(name: &str, force: bool) -> Result<()> {
    let name = normalize_name(name);
    let mut file = read_config_file()?;
    if !file.deployments.contains_key(&name) {
        bail!("Deployment `{name}` is not configured.");
    }
    if file.active_deployment.as_deref() == Some(name.as_str()) && !force {
        bail!(
            "`{name}` is the active deployment. Use --force or switch first with `adpt deployment use <other>`."
        );
    }
    file.deployments.remove(&name);
    if file.active_deployment.as_deref() == Some(name.as_str()) {
        file.active_deployment = file.deployments.keys().next().cloned();
    }
    write_config_file(&file)?;
    delete_api_key(&name)?;
    Ok(())
}
