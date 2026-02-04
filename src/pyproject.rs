use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Default)]
pub struct PyProject {
    pub tool: Option<ToolConfig>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ToolConfig {
    pub adaptive: Option<AdaptiveConfig>,
}

#[derive(Debug, Deserialize, Default)]
pub struct AdaptiveConfig {
    #[serde(rename = "use-case")]
    pub use_case: Option<String>,
    #[serde(rename = "ignore-files", default)]
    pub ignore_files: Vec<String>,
    #[serde(rename = "ignore-extensions", default)]
    pub ignore_extensions: Vec<String>,
    #[serde(default)]
    pub recipes: HashMap<String, RecipeConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RecipeConfig {
    #[serde(rename = "recipe-path")]
    pub recipe_path: Option<String>,
    #[serde(rename = "recipe-key")]
    pub recipe_key: Option<String>,
    #[serde(rename = "use-case")]
    pub use_case: Option<String>,
}

impl PyProject {
    pub fn load(dir: &Path) -> Result<Option<Self>> {
        let pyproject_path = dir.join("pyproject.toml");
        if !pyproject_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&pyproject_path)
            .context("Failed to read pyproject.toml")?;
        let pyproject: PyProject = toml::from_str(&content)
            .context("Failed to parse pyproject.toml")?;
        Ok(Some(pyproject))
    }

    pub fn adaptive_config(&self) -> Option<&AdaptiveConfig> {
        self.tool.as_ref()?.adaptive.as_ref()
    }

    pub fn get_recipe(&self, name: &str) -> Option<&RecipeConfig> {
        self.adaptive_config()?.recipes.get(name)
    }

    pub fn list_recipes(&self) -> Vec<(&String, &RecipeConfig)> {
        self.adaptive_config()
            .map(|c| c.recipes.iter().collect())
            .unwrap_or_default()
    }
}

/// Default files to ignore when packaging recipes
pub const DEFAULT_IGNORE_FILES: &[&str] = &[
    "__pycache__",
    ".ipynb_checkpoints",
    ".venv",
    ".git",
    ".gitignore",
    ".vscode",
    ".ruff_cache",
    ".hypothesis",
    ".pytest_cache",
    "node_modules",
    ".DS_Store",
];

/// Default extensions to ignore when packaging recipes
pub const DEFAULT_IGNORE_EXTENSIONS: &[&str] = &[".pyc", ".ipynb"];

pub fn should_ignore(
    path: &Path,
    ignore_files: &[String],
    ignore_extensions: &[String],
) -> bool {
    // Check file/directory name against ignore list
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        // Check against explicit ignore files
        if ignore_files.iter().any(|f| f == name) {
            return true;
        }
        // Check against default ignore files
        if DEFAULT_IGNORE_FILES.iter().any(|f| *f == name) {
            return true;
        }
    }

    // Check extension
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_with_dot = format!(".{}", ext);
        if ignore_extensions.iter().any(|e| e == &ext_with_dot) {
            return true;
        }
        if DEFAULT_IGNORE_EXTENSIONS.iter().any(|e| *e == ext_with_dot) {
            return true;
        }
    }

    false
}
