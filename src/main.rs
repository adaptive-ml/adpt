use adaptive_client_rust::{AdaptiveClient, UploadEvent};
use anyhow::{Context, Result, anyhow, bail};
use autumnus::{FormatterOption, Options, highlight, themes};
use clap::{Arg, Args, Command, CommandFactory, Parser, Subcommand, ValueHint, value_parser};
use clap_complete::{ArgValueCompleter, CompletionCandidate};
use futures::StreamExt;
use iocraft::prelude::*;
use serde_json::{Map, Value};
use slug::slugify;
use std::{
    env,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};
use tempfile::{NamedTempFile, TempPath, tempdir};
use tokio::{runtime::Handle, sync::watch};
use url::Url;
use uuid::Uuid;
use walkdir::WalkDir;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::pyproject::{PyProject, should_ignore};

// Note: zip_extensions is no longer used but kept in Cargo.toml for potential future use

use crate::{
    json_schema::{JsonSchema, JsonSchemaPropertyContents},
    ui::{
        AllModelsList, ConfigHeader, ErrorMessage, InputPrompt, JobsList, ModelsList, ProgressBar,
        RecipeList, SuccessMessage,
    },
};

mod config;
mod json_schema;
mod pyproject;
mod ui;

const DEFAULT_ADAPTIVE_BASE_URL: &str = "https://app.adaptive.ml";

#[derive(Parser)]
#[command(name = "adpt")]
#[command(version)]
#[command(about = "A tool interacting with the Adaptive platform")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Args)]
struct RunArgs {
    /// Recipe ID or key
    #[arg(add = ArgValueCompleter::new(recipe_key_completer))]
    recipe: String,
    /// A file containing a JSON object of parameters for the recipe
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    parameters: Option<PathBuf>,
    /// The name of the run
    #[arg(short, long)]
    name: Option<String>,
    /// The compute pool to run the recipe on
    #[arg(short, long, add = ArgValueCompleter::new(pool_completer))]
    compute_pool: Option<String>,
    /// The number of GPUs to run the recipe on
    #[arg(short, long)]
    gpus: Option<u32>,
    #[arg(last = true, num_args = 1..)]
    args: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Cancel a job
    Cancel { id: Uuid },
    /// Configure adpt interactively
    Config,
    /// Inspect job
    Job {
        id: Uuid,
        /// Follow job status updates until completion
        #[arg(short, long)]
        follow: bool,
    },
    /// List currently running jobs
    Jobs,
    /// List models
    Models {
        #[arg(short, long, add = ArgValueCompleter::new(usecase_completer))]
        usecase: Option<String>,
        /// List all models in the global model registry
        #[arg(short, long)]
        all: bool,
    },
    /// Upload dataset
    Upload {
        #[arg(short, long, add = ArgValueCompleter::new(usecase_completer))]
        usecase: Option<String>,
        #[arg(value_hint = ValueHint::AnyPath)]
        dataset: PathBuf,
        /// Dataset name
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Upload recipe
    Publish {
        #[arg(short, long, add = ArgValueCompleter::new(usecase_completer))]
        usecase: Option<String>,
        /// Path to recipe file/directory, or recipe name from pyproject.toml
        #[arg(value_hint = ValueHint::AnyPath)]
        recipe: String,
        /// Recipe name (display name)
        #[arg(short, long)]
        name: Option<String>,
        /// Recipe key
        #[arg(short, long)]
        key: Option<String>,
        /// Publish all recipes defined in pyproject.toml
        #[arg(long)]
        all: bool,
        /// List available recipes from pyproject.toml
        #[arg(long)]
        list: bool,
    },
    /// List recipes
    Recipes {
        #[arg(short, long, add = ArgValueCompleter::new(usecase_completer))]
        usecase: Option<String>,
    },
    /// Run recipe
    Run {
        #[arg(short, long, add = ArgValueCompleter::new(usecase_completer))]
        usecase: Option<String>,
        #[command(flatten)]
        args: RunArgs,
    },
    /// Display the schema for inputs for a recipe
    Schema {
        #[arg(short, long, add = ArgValueCompleter::new(usecase_completer))]
        usecase: Option<String>,
        #[arg(add = ArgValueCompleter::new(recipe_key_completer))]
        recipe: String,
    },
    /// Store your API key in the OS keyring
    SetApiKey { api_key: String },
}

fn main() -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _rt_guard = rt.enter();
    clap_complete::CompleteEnv::with_factory(Cli::command).complete();
    let cli = Cli::parse();

    rt.block_on(async {
        match cli.command {
            Commands::Config => interactive_config(),
            Commands::SetApiKey { api_key } => config::set_api_key_keyring(api_key),
            requires_api_key => {
                let config = config::read_config()?;
                let client = AdaptiveClient::new(config.adaptive_base_url, config.adaptive_api_key);
                let default_use_case = config.default_use_case.clone();

                let load_usecase = |maybe_usecase: Option<String>| {
                    maybe_usecase.or(default_use_case.clone()).expect(
                        "A usecase must be specified via the --usecase argument or a default usecase configured"
                    )
                };

                match requires_api_key {
                    Commands::Recipes { usecase } => {
                                        list_recipes(&client, &load_usecase(usecase)).await
                                    }
                    Commands::Job { id, follow } => get_job(Arc::new(client), id, follow).await,
                    Commands::Publish {
                                        usecase,
                                        recipe,
                                        name,
                                        key,
                                        all,
                                        list,
                                    } => publish_recipe_cmd(&client, usecase, default_use_case.clone(), name, key, recipe, all, list).await,
                    Commands::Run { usecase, args } => {
                                        run_recipe(&client, &load_usecase(usecase), args).await
                                    }
                    Commands::Jobs => list_jobs(&client, None).await,
                    Commands::Cancel { id } => cancel_job(&client, id).await,
                    Commands::Models { usecase, all } => {
                                        if all {
                                            list_all_models(&client).await
                                        } else {
                                            match usecase.or(config.default_use_case) {
                                                Some(use_case) => list_models(&client, use_case).await,
                                                None => list_all_models(&client).await,
                                            }
                                        }
                                    }
                    Commands::Schema { usecase, recipe } => {
                                        print_schema(&client, load_usecase(usecase), recipe).await
                                    }
                    Commands::Config => panic!("This state should be unreachable"),
                    Commands::SetApiKey { api_key: _ } => panic!("This state should be unreachable"),
                    Commands::Upload { usecase, dataset, name } => upload_dataset(&client, &load_usecase(usecase), dataset, name).await,
                }
            },
        }
    })
}

async fn upload_dataset<P: AsRef<Path> + Sync>(
    client: &AdaptiveClient,
    usecase: &str,
    dataset: P,
    name: Option<String>,
) -> std::result::Result<(), anyhow::Error> {
    let file_size = std::fs::metadata(dataset.as_ref())
        .context("Failed to get file metadata")?
        .len();

    let name = name.unwrap_or_else(|| {
        let file_name = dataset.as_ref().file_name().unwrap().to_string_lossy();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("SystemTime before UNIX EPOCH");
        format!("{}-{}", file_name, now.as_secs())
    });

    if file_size > adaptive_client_rust::MIN_CHUNK_SIZE_BYTES {
        let key = slugify(&name);
        let mut stream = client.chunked_upload_dataset(usecase, &name, &key, &dataset)?;

        let (tx, rx) = watch::channel(0.0);

        let process_stream = async {
            let mut response = None;
            while let Some(event) = stream.next().await {
                match event? {
                    UploadEvent::Progress(p) => {
                        let percent = (p.bytes_uploaded as f32 / p.total_bytes as f32) * 100.0;
                        let _ = tx.send(percent);
                    }
                    UploadEvent::Complete(r) => {
                        response = Some(r);
                        break;
                    }
                }
            }
            Ok::<_, anyhow::Error>(response.expect("Stream ended without Complete event"))
        };

        let mut progress_bar =
            element!(ProgressBar(title: "Uploading Dataset".to_string(), progress: Some(rx)));

        let response = tokio::select! {
            result = process_stream => result?,
            _ = progress_bar.render_loop() => {
                unreachable!("render_loop should not terminate")
            }
        };

        println!(
            "Dataset uploaded successfully with ID: {}",
            response.dataset_id,
        );
    } else {
        let response = client.upload_dataset(usecase, &name, &dataset).await?;

        println!(
            "Dataset uploaded successfully with ID: {}, key: {}",
            response.id,
            response.key.unwrap_or("<none>".to_string())
        );
    }

    Ok(())
}

async fn print_schema(client: &AdaptiveClient, usecase: String, recipe: String) -> Result<()> {
    let recipe = client
        .get_recipe(usecase, recipe)
        .await?
        .ok_or_else(|| anyhow!("Recipe not found"))?;
    let output = highlight(
        &serde_json::to_string_pretty(&recipe.json_schema)?,
        Options {
            formatter: FormatterOption::Terminal {
                theme: Some(themes::get("ayu_light").expect("Syntax highlighting theme not found")),
            },
            lang_or_file: Some("json"),
        },
    );
    println!("{}", output);
    Ok(())
}

async fn list_models(client: &AdaptiveClient, usecase: String) -> Result<()> {
    let model_services = client.list_models(usecase).await?;
    element!(ModelsList(model_services: model_services)).print();
    Ok(())
}

async fn list_all_models(client: &AdaptiveClient) -> Result<()> {
    let models = client.list_all_models().await?;
    element!(AllModelsList(models: models)).print();
    Ok(())
}

async fn cancel_job(client: &AdaptiveClient, id: Uuid) -> Result<()> {
    let cancelled = client.cancel_job(id).await?;
    println!("Job {} cancelled successfully", cancelled.id);
    Ok(())
}

async fn get_job(client: Arc<AdaptiveClient>, job_id: Uuid, follow: bool) -> Result<()> {
    if follow {
        element! {
            ui::FollowJobStatus(client: Some(client.clone()), job_id: job_id)
        }
        .render_loop()
        .await
        .unwrap();
    } else {
        let job = client.get_job(job_id).await?;
        element! {ui::JobStatus(stages: job.stages, name: job.name, status: job.status.to_string(), error: job.error)}.print();
    }

    Ok(())
}

async fn list_recipes(client: &AdaptiveClient, usecase: &str) -> Result<()> {
    let recipes = client.list_recipes(usecase).await?;

    element!(RecipeList(recipes: recipes)).print();

    Ok(())
}

/// Prepare a recipe directory for upload by copying to temp dir with filtering
fn prepare_recipe_dir(
    source_dir: &Path,
    recipe_path: Option<&str>,
    ignore_files: &[String],
    ignore_extensions: &[String],
) -> Result<tempfile::TempDir> {
    let temp_dir = tempdir()?;
    let dest_dir = temp_dir.path();

    // Walk the source directory and copy files, respecting ignore patterns
    for entry in WalkDir::new(source_dir) {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(source_dir)?;

        // Skip if any component of the path should be ignored
        let should_skip = relative.components().any(|c| {
            let component_path = Path::new(c.as_os_str());
            should_ignore(component_path, ignore_files, ignore_extensions)
        });

        if should_skip {
            continue;
        }

        let dest_path = dest_dir.join(relative);

        if path.is_dir() {
            fs::create_dir_all(&dest_path)?;
        } else if path.is_file() {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &dest_path)?;
        }
    }

    // If recipe_path is specified, copy it to main.py
    if let Some(rp) = recipe_path {
        let recipe_file = source_dir.join(rp);
        if !recipe_file.exists() {
            bail!("Recipe file not found: {}", recipe_file.display());
        }
        let main_py = dest_dir.join("main.py");
        fs::copy(&recipe_file, &main_py)?;
        println!("Copied {} to main.py", rp);
    }

    // Clean up pyproject.toml for server deployment
    let pyproject_path = dest_dir.join("pyproject.toml");
    if pyproject_path.exists() {
        let content = fs::read_to_string(&pyproject_path)?;
        if let Ok(mut doc) = content.parse::<toml::Table>() {
            let mut modified = false;

            // Remove [tool.uv] section (sources and index) - server has its own config
            if let Some(tool) = doc.get_mut("tool").and_then(|t| t.as_table_mut()) {
                if tool.remove("uv").is_some() {
                    println!("Removed [tool.uv] from pyproject.toml");
                    modified = true;
                }
            }

            if modified {
                let new_content = toml::to_string_pretty(&doc)?;
                fs::write(&pyproject_path, new_content)?;
            }
        }
    }

    // Verify main.py exists
    if !dest_dir.join("main.py").exists() {
        bail!(
            "Recipe directory must contain a main.py file (or specify recipe-path in pyproject.toml)"
        );
    }

    Ok(temp_dir)
}

fn zip_recipe_dir(recipe_dir: &Path) -> Result<TempPath> {
    let tmp_file = NamedTempFile::new()?;

    {
        let mut zip = ZipWriter::new(&tmp_file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        for entry in WalkDir::new(recipe_dir) {
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(recipe_dir)?;

            if relative.as_os_str().is_empty() {
                continue;
            }

            if path.is_file() {
                zip.start_file(relative.to_string_lossy(), options)?;
                let content = fs::read(path)?;
                std::io::Write::write_all(&mut zip, &content)?;
            } else if path.is_dir() {
                zip.add_directory(relative.to_string_lossy(), options)?;
            }
        }

        zip.finish()?;
    }

    Ok(tmp_file.into_temp_path())
}

async fn publish_single_recipe(
    client: &AdaptiveClient,
    usecase: &str,
    name: Option<String>,
    key: Option<String>,
    source_dir: &Path,
    recipe_path: Option<&str>,
    ignore_files: &[String],
    ignore_extensions: &[String],
) -> Result<()> {
    // Prepare the recipe directory
    let temp_dir = prepare_recipe_dir(source_dir, recipe_path, ignore_files, ignore_extensions)?;

    // Generate name and key
    let name = name.unwrap_or_else(|| {
        let dir_name = source_dir.file_name().unwrap().to_string_lossy();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("SystemTime before UNIX EPOCH");
        format!("{}-{}", dir_name, now.as_secs())
    });
    let key = key.unwrap_or_else(|| slugify(&name));

    // Zip and upload
    let zip_path = zip_recipe_dir(temp_dir.path())?;

    let response = client
        .publish_recipe(usecase, &name, &key, &zip_path)
        .await?;

    println!(
        "Recipe published successfully with ID: {}, key: {}",
        response.id,
        response.key.unwrap_or_else(|| "<none>".to_string())
    );

    Ok(())
}

async fn publish_recipe_cmd(
    client: &AdaptiveClient,
    usecase: Option<String>,
    default_use_case: Option<String>,
    name: Option<String>,
    key: Option<String>,
    recipe: String,
    all: bool,
    list: bool,
) -> Result<()> {
    let cwd = env::current_dir()?;
    let pyproject = PyProject::load(&cwd)?;

    // Handle --list flag
    if list {
        match &pyproject {
            Some(pp) => {
                let recipes = pp.list_recipes();
                if recipes.is_empty() {
                    println!("No recipes found in pyproject.toml");
                } else {
                    println!("Available recipes:");
                    for (name, cfg) in recipes {
                        let key = cfg.recipe_key.as_deref().unwrap_or("(no recipe-key)");
                        let uc = cfg
                            .use_case
                            .as_ref()
                            .or(pp.adaptive_config().and_then(|c| c.use_case.as_ref()))
                            .map(|s| s.as_str())
                            .unwrap_or("(no use-case)");
                        println!("  - {}: {} ({})", name, key, uc);
                    }
                }
            }
            None => println!("No pyproject.toml found in current directory"),
        }
        return Ok(());
    }

    // Get ignore patterns from pyproject.toml
    let (ignore_files, ignore_extensions) = pyproject
        .as_ref()
        .and_then(|pp| pp.adaptive_config())
        .map(|c| (c.ignore_files.clone(), c.ignore_extensions.clone()))
        .unwrap_or_default();

    // Handle --all flag
    if all {
        let pp = pyproject.as_ref().ok_or_else(|| {
            anyhow!("No pyproject.toml found in current directory")
        })?;
        let recipes = pp.list_recipes();
        if recipes.is_empty() {
            bail!("No recipes found in pyproject.toml");
        }

        let recipe_count = recipes.len();
        println!("Publishing {} recipe(s)...\n", recipe_count);
        for (recipe_name, cfg) in recipes {
            let recipe_key = cfg
                .recipe_key
                .as_ref()
                .ok_or_else(|| anyhow!("recipe-key not defined for recipe '{}'", recipe_name))?;

            let uc = cfg
                .use_case
                .as_ref()
                .or(pp.adaptive_config().and_then(|c| c.use_case.as_ref()))
                .or(usecase.as_ref())
                .or(default_use_case.as_ref())
                .ok_or_else(|| anyhow!("use-case not defined for recipe '{}'", recipe_name))?;

            println!("Publishing recipe: {} (key: {}, use-case: {})", recipe_name, recipe_key, uc);
            publish_single_recipe(
                client,
                uc,
                Some(recipe_name.clone()),
                Some(recipe_key.clone()),
                &cwd,
                cfg.recipe_path.as_deref(),
                &ignore_files,
                &ignore_extensions,
            )
            .await?;
            println!();
        }
        println!("Successfully published {} recipe(s)!", recipe_count);
        return Ok(());
    }

    // Check if recipe is a name from pyproject.toml
    if let Some(pp) = &pyproject {
        if let Some(cfg) = pp.get_recipe(&recipe) {
            let recipe_key = cfg
                .recipe_key
                .as_ref()
                .or(key.as_ref())
                .ok_or_else(|| anyhow!("recipe-key not defined for recipe '{}'", recipe))?;

            let uc = cfg
                .use_case
                .as_ref()
                .or(pp.adaptive_config().and_then(|c| c.use_case.as_ref()))
                .or(usecase.as_ref())
                .or(default_use_case.as_ref())
                .ok_or_else(|| anyhow!("use-case not defined for recipe '{}'", recipe))?;

            println!(
                "Publishing recipe: {} (key: {}, use-case: {})",
                recipe, recipe_key, uc
            );
            return publish_single_recipe(
                client,
                uc,
                name.or(Some(recipe.clone())),
                Some(recipe_key.clone()),
                &cwd,
                cfg.recipe_path.as_deref(),
                &ignore_files,
                &ignore_extensions,
            )
            .await;
        }
    }

    // Fall back to treating recipe as a path
    let recipe_path = PathBuf::from(&recipe);
    if !recipe_path.exists() {
        bail!(
            "Recipe '{}' not found as a pyproject.toml recipe name or file path",
            recipe
        );
    }

    let uc = usecase
        .or(default_use_case)
        .ok_or_else(|| anyhow!("A usecase must be specified via --usecase or configured as default"))?;

    if recipe_path.is_dir() {
        publish_single_recipe(
            client,
            &uc,
            name,
            key,
            &recipe_path,
            None,
            &ignore_files,
            &ignore_extensions,
        )
        .await
    } else {
        // Single file - upload directly
        let response = client
            .publish_recipe(&uc, &name.unwrap_or(recipe.clone()), &key.unwrap_or_else(|| slugify(&recipe)), &recipe_path)
            .await?;
        println!(
            "Recipe published successfully with ID: {}, key: {}",
            response.id,
            response.key.unwrap_or_else(|| "<none>".to_string())
        );
        Ok(())
    }
}

fn recipe_key_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let mut completions = vec![];
    let Some(current) = current.to_str() else {
        return completions;
    };

    let config = config::read_config().expect("Failed to read config");

    let client = AdaptiveClient::new(config.adaptive_base_url, config.adaptive_api_key);

    let handle = Handle::current();
    let recipes = handle
        .block_on(client.list_recipes(&config.default_use_case.expect("No default usecase set")))
        .unwrap();

    recipes.into_iter().for_each(|recipe| {
        if let Some(key) = recipe.key
            && key.starts_with(current)
        {
            completions.push(CompletionCandidate::new(key));
        }
    });

    completions
}

fn usecase_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let mut completions = vec![];
    let Some(current) = current.to_str() else {
        return completions;
    };

    let config = config::read_config().expect("Failed to read config");

    let client = AdaptiveClient::new(config.adaptive_base_url, config.adaptive_api_key);

    let handle = Handle::current();
    let usecases = handle.block_on(client.list_usecases()).unwrap();

    usecases.into_iter().for_each(|usecase| {
        if usecase.key.starts_with(current) {
            completions.push(CompletionCandidate::new(usecase.key));
        }
    });

    completions
}

fn pool_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let mut completions = vec![];
    let Some(current) = current.to_str() else {
        return completions;
    };

    let config = config::read_config().expect("Failed to read config");

    let client = AdaptiveClient::new(config.adaptive_base_url, config.adaptive_api_key);

    let handle = Handle::current();
    let pools = handle.block_on(client.list_pools()).unwrap();

    pools.into_iter().for_each(|pool| {
        if pool.key.starts_with(current) {
            completions.push(CompletionCandidate::new(pool.key));
        }
    });

    completions
}

async fn parse_recipe_args(
    client: &AdaptiveClient,
    usecase: &str,
    recipe: String,
    args: Vec<String>,
) -> Result<Map<String, Value>> {
    let recipe_contents = client
        .get_recipe(usecase.to_string(), recipe.clone())
        .await?
        .ok_or_else(|| anyhow!("Recipe not found"))?;
    let schema = recipe_contents.json_schema;
    let schema: JsonSchema =
        serde_json::from_value(schema).map_err(|e| anyhow!("Failed to parse JSON schema: {e}"))?;

    let expected_args = schema
        .properties
        .iter()
        .map(|(name, value)| match value {
            JsonSchemaPropertyContents::Regular(regular_json_schema_property_contents) => {
                let base = Arg::new(name)
                    .required(schema.required.contains(name))
                    .help(regular_json_schema_property_contents.description.clone())
                    .long(name);

                match regular_json_schema_property_contents.type_.as_str() {
                    "integer" => Ok(base.value_parser(value_parser!(i64))),
                    "string" => Ok(base.value_parser(value_parser!(String))),
                    "boolean" => Ok(base.value_parser(value_parser!(bool))),
                    "number" => Ok(base.value_parser(value_parser!(f64))),
                    unknown => Err(anyhow!("Unknown type {unknown} specified in schema")),
                }
            }
            JsonSchemaPropertyContents::Union(_) => Ok(Arg::new(name).required(true).long(name)),
        })
        .collect::<Result<Vec<_>>>()?;

    let command = Command::new(format!("adpt run {} --", recipe))
        .args(expected_args)
        .no_binary_name(true);

    let parsed_result = command.try_get_matches_from(args);

    let parsed_args = match parsed_result {
        Ok(result) => result,
        Err(e) => e.exit(),
    };

    let mut parameters = Map::new();
    for (name, value) in schema.properties {
        match value {
            JsonSchemaPropertyContents::Regular(regular_json_schema_property_contents) => {
                match regular_json_schema_property_contents.type_.as_str() {
                    "integer" => {
                        if let Some(value) = parsed_args.get_one::<i64>(&name) {
                            let v = serde_json::to_value(value).unwrap();
                            parameters.insert(name.clone(), v);
                        }
                    }
                    "string" => {
                        if let Some(value) = parsed_args.get_one::<String>(&name) {
                            let v = serde_json::to_value(value).unwrap();
                            parameters.insert(name.clone(), v);
                        }
                    }
                    "boolean" => {
                        if let Some(value) = parsed_args.get_one::<bool>(&name) {
                            let v = serde_json::to_value(value).unwrap();
                            parameters.insert(name.clone(), v);
                        }
                    }
                    "number" => {
                        if let Some(value) = parsed_args.get_one::<f64>(&name) {
                            let v = serde_json::to_value(value).unwrap();
                            parameters.insert(name.clone(), v);
                        }
                    }

                    _ => (),
                }
            }
            JsonSchemaPropertyContents::Union(_) => {
                if let Some(value) = parsed_args.get_one::<String>(&name) {
                    //FIXME so provide a arg validator that checks for json
                    let v = serde_json::from_str(value).unwrap();
                    parameters.insert(name.clone(), v);
                }
            }
        }
    }
    Ok(parameters)
}

async fn run_recipe(client: &AdaptiveClient, usecase: &str, run_args: RunArgs) -> Result<()> {
    let parameters = if let Some(parameters_file) = run_args.parameters {
        let content = fs::read_to_string(&parameters_file)?;
        serde_json::from_str(&content).map_err(|e| {
            anyhow!(
                "Failed to parse parameters: {e} from file {}",
                parameters_file.clone().to_str().unwrap()
            )
        })?
    } else if run_args.recipe.is_empty() {
        Map::new()
    } else {
        parse_recipe_args(client, usecase, run_args.recipe.clone(), run_args.args).await?
    };

    let response = client
        .run_recipe(
            usecase,
            &run_args.recipe.to_string(),
            parameters,
            run_args.name,
            run_args.compute_pool,
            run_args.gpus.unwrap_or(1),
        )
        .await?;

    println!("Recipe run successfully with ID: {}", response.id);

    Ok(())
}

async fn list_jobs(client: &AdaptiveClient, usecase: Option<String>) -> Result<()> {
    let response = client.list_jobs(usecase).await?;

    element!(JobsList(jobs: response)).print();

    Ok(())
}

fn read_input(prompt: &str, default: Option<&str>, description: Option<&str>) -> Result<String> {
    element! {
        InputPrompt(
            prompt: prompt.to_string(),
            default: default.map(|s| s.to_string()),
            description: description.map(|s| s.to_string())
        )
    }
    .print();

    print!("> ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_string();

    if input.is_empty() {
        if let Some(def) = default {
            Ok(def.to_string())
        } else {
            Ok(input)
        }
    } else {
        Ok(input)
    }
}

fn interactive_config() -> Result<()> {
    element!(ConfigHeader()).print();

    let adaptive_base_url = loop {
        let base_url_str = read_input(
            "Adaptive Base URL",
            Some(DEFAULT_ADAPTIVE_BASE_URL),
            Some("The base URL for your Adaptive instance"),
        )?;

        match Url::parse(&base_url_str) {
            Ok(url) => break url,
            Err(e) => {
                element!(ErrorMessage(message: format!("Invalid URL: {}", e))).print();
                println!();
            }
        }
    };

    let adaptive_api_key = loop {
        let api_key = read_input(
            "API Key",
            None,
            Some("Your Adaptive API key (stored securely in OS keyring)"),
        )?;

        if api_key.is_empty() {
            element!(ErrorMessage(message: "API key cannot be empty".to_string())).print();
            println!();
        } else {
            break api_key;
        }
    };

    let default_use_case_str = read_input(
        "Default Use Case",
        None,
        Some("Optional: Set a default use case to avoid specifying --usecase every time"),
    )?;
    let default_use_case = if default_use_case_str.is_empty() {
        None
    } else {
        Some(default_use_case_str)
    };

    config::set_api_key_keyring(adaptive_api_key)?;

    let config_file = config::ConfigFile {
        adaptive_base_url: Some(adaptive_base_url),
        default_use_case,
    };

    config::write_config(config_file)?;

    element!(SuccessMessage(message: "Configuration complete!".to_string())).print();

    Ok(())
}
