use anyhow::{Context, Result, anyhow, bail};
use autumnus::{FormatterOption, Options, highlight, themes};
use clap::{Arg, Args, Command, CommandFactory, Parser, Subcommand, ValueHint, value_parser};
use clap_complete::{ArgValueCompleter, CompletionCandidate};
use client::AdaptiveClient;
use iocraft::prelude::*;
use serde_json::{Map, Value};
use slug::slugify;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};
use tempfile::{NamedTempFile, TempPath};
use tokio::runtime::Handle;
use url::Url;
use uuid::Uuid;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use zip_extensions::write::ZipWriterExtensions;

use crate::{
    json_schema::{JsonSchema, JsonSchemaPropertyContents},
    ui::{
        AllModelsList, ConfigHeader, ErrorMessage, InputPrompt, JobsList, ModelsList, RecipeList,
        SuccessMessage,
    },
};

mod client;
mod config;
mod json_schema;
mod rest_types;
mod serde_utils;
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
        #[arg(value_hint = ValueHint::AnyPath)]
        recipe: PathBuf,
        /// Recipe name
        #[arg(short, long)]
        name: Option<String>,
        /// Recipe key
        #[arg(short, long)]
        key: Option<String>,
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
                                    } => publish_recipe(&client, &load_usecase(usecase), name, key, recipe).await,
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

async fn upload_dataset<P: AsRef<Path>>(
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

    if file_size > client::MIN_CHUNK_SIZE_BYTES {
        let key = slugify(&name);
        let response = client
            .chunked_upload_dataset(usecase, &name, &key, &dataset)
            .await?;

        println!(
            "Dataset uploaded successfully with ID: {}, key: {}",
            response.id,
            response.key.unwrap_or("<none>".to_string())
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

fn zip_recipe_dir<P: AsRef<Path>>(recipe_dir: P) -> Result<TempPath> {
    if recipe_dir.as_ref().join("main.py").is_file() {
        let tmp_file = NamedTempFile::new()?;
        let zip_file = ZipWriter::new(&tmp_file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip_file
            .create_from_directory_with_options(&recipe_dir.as_ref().to_owned(), |_| options)?;
        Ok(tmp_file.into_temp_path())
    } else {
        bail!("Recipe directory must contain a main.py file");
    }
}

async fn publish_recipe<P: AsRef<Path>>(
    client: &AdaptiveClient,
    usecase: &str,
    name: Option<String>,
    key: Option<String>,
    recipe: P,
) -> Result<()> {
    let name = name.unwrap_or_else(|| {
        let file_name = recipe.as_ref().file_name().unwrap().to_string_lossy();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("SystemTime before UNIX EPOCH");
        format!("{}-{}", file_name, now.as_secs())
    });
    let key = key.unwrap_or_else(|| slugify(&name));

    let response = if recipe.as_ref().is_dir() {
        let recipe = zip_recipe_dir(recipe)?;
        client.publish_recipe(usecase, &name, &key, &recipe).await?
    } else {
        client.publish_recipe(usecase, &name, &key, recipe).await?
    };

    println!(
        "Recipe published successfully with ID: {}, key: {}",
        response.id,
        response.key.unwrap_or("<none>".to_string())
    );

    Ok(())
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
