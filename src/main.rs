use adaptive_client_rust::{AdaptiveClient, UploadEvent, create_user};
use anyhow::{Context, Result, anyhow, bail};
use autumnus::{FormatterOption, Options, highlight, themes};
use clap::{
    Arg, Args, Command, CommandFactory, Parser, Subcommand, ValueEnum, ValueHint, value_parser,
};
use clap_complete::{ArgValueCompleter, CompletionCandidate};
use email_address::EmailAddress;
use futures::StreamExt;
use iocraft::prelude::*;
use serde_json::{Map, Value};
use slug::slugify;
use std::{
    fs,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};
use tempfile::{NamedTempFile, TempPath};
use tokio::{runtime::Handle, sync::watch};
use url::Url;
use uuid::Uuid;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use zip_extensions::{
    zip_ignore_entry_handler::ZipIgnoreEntryHandler, zip_writer_extensions::ZipWriterExtensions,
};

use crate::{
    json_schema::{JsonSchema, JsonSchemaPropertyContents},
    terminal::TitleGuard,
    ui::{
        AllModelsList, Cell, Column, ConfigHeader, ErrorMessage, InputPrompt, JobsList, ListConfig,
        ModelsList, ProgressBar, RecipeList, SuccessMessage, render_list,
    },
};

mod config;
mod json_schema;
mod terminal;
mod ui;

const DEFAULT_ADAPTIVE_BASE_URL: &str = "https://app.adaptive.ml";

#[derive(Parser)]
#[command(name = "adpt")]
#[command(version)]
#[command(about = "A tool interacting with the Adaptive platform")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(long, hide = true)]
    markdown_help: bool,
}

#[derive(Args)]
struct RunArgs {
    /// Recipe ID or key
    #[arg(add = ArgValueCompleter::new(recipe_key_completer))]
    recipe: String,
    /// A file containing a JSON object of parameters for the recipe
    #[arg(long, value_hint = ValueHint::FilePath)]
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
enum RoleCommands {
    /// Create a new role
    Create {
        /// Role name
        name: String,
        /// Role key (auto-generated from name if not provided)
        #[arg(short, long)]
        key: Option<String>,
        /// Permissions to assign to the role
        #[arg(short, long, required = true, num_args = 1..)]
        permissions: Vec<String>,
    },
    /// Describe a role
    Describe {
        /// Role ID (UUID) or key
        id_or_key: String,
    },
    /// List all roles
    List,
    /// Add permissions to a role
    AddPermission {
        /// Role ID or key
        role: String,
        /// Permissions to add
        #[arg(required = true, num_args = 1..)]
        permissions: Vec<String>,
    },
    /// Remove permissions from a role
    RemovePermission {
        /// Role ID or key
        role: String,
        /// Permissions to remove
        #[arg(required = true, num_args = 1..)]
        permissions: Vec<String>,
    },
}

#[derive(Clone, ValueEnum)]
enum UserTypeArg {
    Human,
    System,
}

impl From<UserTypeArg> for create_user::UserType {
    fn from(arg: UserTypeArg) -> Self {
        match arg {
            UserTypeArg::Human => Self::HUMAN,
            UserTypeArg::System => Self::SYSTEM,
        }
    }
}

#[derive(Subcommand)]
enum UserCommands {
    /// Create a new user
    Create {
        /// User name
        name: String,
        /// User email (required for human users)
        #[arg(short, long, value_hint = ValueHint::EmailAddress)]
        email: Option<EmailAddress>,
        /// User type
        #[arg(short = 't', long, default_value = "human")]
        user_type: UserTypeArg,
    },
    /// Delete a user
    Delete {
        /// User ID or email
        id_or_email: String,
    },
    /// Describe a user
    Describe {
        /// User ID or email
        id_or_email: String,
    },
    /// List all users
    List,
}

#[derive(Subcommand)]
enum TeamCommands {
    /// Create a new team
    Create {
        /// Team name
        name: String,
        /// Team key (auto-generated from name if not provided)
        #[arg(short, long)]
        key: Option<String>,
    },
    /// Add a user to a team
    AddMember {
        /// User ID or email
        user: String,
        /// Team ID or key
        team: String,
        /// Role ID or key
        role: String,
    },
    /// Remove a user from a team
    RemoveMember {
        /// User ID or email
        user: String,
        /// Team ID or key
        team: String,
    },
    /// List all teams
    List,
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
        #[arg(short, long, add = ArgValueCompleter::new(project_completer))]
        project: Option<String>,
        /// List all models in the global model registry
        #[arg(short, long)]
        all: bool,
    },
    /// Upload dataset
    Upload {
        #[arg(short, long, add = ArgValueCompleter::new(project_completer))]
        project: Option<String>,
        #[arg(value_hint = ValueHint::AnyPath)]
        dataset: PathBuf,
        /// Dataset name
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Upload recipe
    Publish {
        #[arg(short, long, add = ArgValueCompleter::new(project_completer))]
        project: Option<String>,
        #[arg(value_hint = ValueHint::AnyPath)]
        recipe: PathBuf,
        /// Recipe name
        #[arg(short, long)]
        name: Option<String>,
        /// Recipe key
        #[arg(short, long)]
        key: Option<String>,
        /// Custom entrypoint file (relative path within directory)
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        entrypoint: Option<String>,
        /// Custom config entrypoint file (relative path within directory)
        #[arg(short = 'c', long, value_hint = ValueHint::FilePath)]
        entrypoint_config: Option<String>,
        /// Update existing recipe if it exists
        #[arg(short, long)]
        force: bool,
    },
    /// List recipes
    Recipes {
        #[arg(short, long, add = ArgValueCompleter::new(project_completer))]
        project: Option<String>,
    },
    /// Run recipe
    Run {
        #[arg(short, long, add = ArgValueCompleter::new(project_completer))]
        project: Option<String>,
        #[command(flatten)]
        args: RunArgs,
    },
    /// Display the schema for inputs for a recipe
    Schema {
        #[arg(short, long, add = ArgValueCompleter::new(project_completer))]
        project: Option<String>,
        #[arg(add = ArgValueCompleter::new(recipe_key_completer))]
        recipe: String,
    },
    /// Store your API key in the OS keyring
    SetApiKey { api_key: String },
    /// Manage roles
    Role {
        #[command(subcommand)]
        command: RoleCommands,
    },
    /// Manage users
    User {
        #[command(subcommand)]
        command: UserCommands,
    },
    /// Manage teams
    Team {
        #[command(subcommand)]
        command: TeamCommands,
    },
}

impl Commands {
    fn name(&self) -> &'static str {
        match self {
            Commands::Cancel { .. } => "cancel",
            Commands::Config => "config",
            Commands::Job { .. } => "job",
            Commands::Jobs => "jobs",
            Commands::Models { .. } => "models",
            Commands::Upload { .. } => "upload",
            Commands::Publish { .. } => "publish",
            Commands::Recipes { .. } => "recipes",
            Commands::Run { .. } => "run",
            Commands::Schema { .. } => "schema",
            Commands::SetApiKey { .. } => "set-api-key",
            Commands::Role { .. } => "role",
            Commands::User { .. } => "user",
            Commands::Team { .. } => "team",
        }
    }
}

fn main() -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _rt_guard = rt.enter();
    clap_complete::CompleteEnv::with_factory(Cli::command).complete();
    let cli = Cli::parse();
    if cli.markdown_help {
        clap_markdown::print_help_markdown::<Cli>();
        return Ok(());
    }
    let _title_guard = TitleGuard::new(&format!("adpt - {}", cli.command.name()));

    rt.block_on(async {
        match cli.command {
            Commands::Config => interactive_config(),
            Commands::SetApiKey { api_key } => config::set_api_key_keyring(api_key),
            requires_api_key => {
                let config = config::read_config()?;
                let client = AdaptiveClient::new(config.adaptive_base_url, config.adaptive_api_key);
                let default_project = config.default_project.clone();

                let load_project = |maybe_project: Option<String>| {
                    maybe_project.or(default_project.clone()).expect(
                        "A project must be specified via the --project argument or a default project configured"
                    )
                };

                match requires_api_key {
                    Commands::Recipes { project } => {
                                        list_recipes(&client, &load_project(project)).await
                                    }
                    Commands::Job { id, follow } => get_job(Arc::new(client), id, follow).await,
                    Commands::Publish {
                                        project,
                                        recipe,
                                        name,
                                        key,
                                        entrypoint,
                                        entrypoint_config,
                                        force,
                                    } => publish_recipe(&client, &load_project(project), name, key, recipe, entrypoint, entrypoint_config, force).await,
                    Commands::Run { project, args } => {
                                        run_recipe(&client, &load_project(project), args).await
                                    }
                    Commands::Jobs => list_jobs(&client, None).await,
                    Commands::Cancel { id } => cancel_job(&client, id).await,
                    Commands::Models { project, all } => {
                                        if all {
                                            list_all_models(&client).await
                                        } else {
                                            match project.or(config.default_project) {
                                                Some(project) => list_models(&client, project).await,
                                                None => list_all_models(&client).await,
                                            }
                                        }
                                    }
                    Commands::Schema { project, recipe } => {
                                        print_schema(&client, load_project(project), recipe).await
                                    }
                    Commands::Config => panic!("This state should be unreachable"),
                    Commands::SetApiKey { api_key: _ } => panic!("This state should be unreachable"),
                    Commands::Upload { project, dataset, name } => upload_dataset(&client, &load_project(project), dataset, name).await,
                    Commands::Role { command } => match command {
                        RoleCommands::Create { name, key, permissions } => {
                            create_role(&client, &name, key.as_deref(), permissions).await
                        }
                        RoleCommands::Describe { id_or_key } => {
                            describe_role(&client, &id_or_key).await
                        }
                        RoleCommands::List => list_roles(&client).await,
                        RoleCommands::AddPermission { role, permissions } => {
                            add_role_permission(&client, &role, permissions).await
                        }
                        RoleCommands::RemovePermission { role, permissions } => {
                            remove_role_permission(&client, &role, permissions).await
                        }
                    },
                    Commands::User { command } => match command {
                        UserCommands::Create { name, email, user_type } => {
                            create_user(&client, &name, email, user_type).await
                        }
                        UserCommands::Delete { id_or_email } => {
                            delete_user(&client, &id_or_email).await
                        }
                        UserCommands::Describe { id_or_email } => {
                            describe_user(&client, &id_or_email).await
                        }
                        UserCommands::List => list_users(&client).await,
                    },
                    Commands::Team { command } => match command {
                        TeamCommands::Create { name, key } => {
                            create_team(&client, &name, key.as_deref()).await
                        }
                        TeamCommands::AddMember { user, team, role } => {
                            add_team_member(&client, &user, &team, &role).await
                        }
                        TeamCommands::RemoveMember { user, team } => {
                            remove_team_member(&client, &user, &team).await
                        }
                        TeamCommands::List => list_teams(&client).await,
                    },
                }
            },
        }
    })
}

async fn upload_dataset<P: AsRef<Path> + Sync>(
    client: &AdaptiveClient,
    project: &str,
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
        let mut stream = client.chunked_upload_dataset(project, &name, &key, &dataset)?;

        terminal::set_progress(terminal::Progress::SetPercentage(0));
        let (tx, rx) = watch::channel(0.0);

        let process_stream = async {
            let mut response = None;
            while let Some(event) = stream.next().await {
                match event? {
                    UploadEvent::Progress(p) => {
                        let percent = (p.bytes_uploaded as f32 / p.total_bytes as f32) * 100.0;
                        let _ = tx.send(percent);
                        terminal::set_progress(terminal::Progress::SetPercentage(percent as u8));
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

        terminal::set_progress(terminal::Progress::None);
        if io::stdout().is_terminal() {
            println!(
                "Dataset uploaded successfully with ID: {}",
                response.dataset_id,
            );
        } else {
            println!("{}", response.dataset_id);
        }
        terminal::send_notification("Dataset upload complete");
    } else {
        terminal::set_progress(terminal::Progress::SetIndeterminate);
        let response = client.upload_dataset(project, &name, &dataset).await?;

        if io::stdout().is_terminal() {
            println!(
                "Dataset uploaded successfully with ID: {}, key: {}",
                response.id,
                response.key.unwrap_or("<none>".to_string())
            );
        } else {
            println!("{}", response.id)
        }
        terminal::send_notification("Dataset upload complete");
    }
    terminal::set_progress(terminal::Progress::None);
    terminal::send_notification("Dataset upload complete");

    Ok(())
}

async fn print_schema(client: &AdaptiveClient, project: String, recipe: String) -> Result<()> {
    let recipe = client
        .get_recipe(project, recipe)
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

async fn list_models(client: &AdaptiveClient, project: String) -> Result<()> {
    let model_services = client.list_models(project).await?;
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

async fn list_recipes(client: &AdaptiveClient, project: &str) -> Result<()> {
    let recipes = client.list_recipes(project).await?;

    element!(RecipeList(recipes: recipes)).print();

    Ok(())
}

fn zip_recipe_dir<P: AsRef<Path>>(recipe_dir: P, entrypoint: &Option<String>) -> Result<TempPath> {
    if entrypoint.is_none() && !recipe_dir.as_ref().join("main.py").is_file() {
        bail!("Recipe directory must contain a main.py file, or specify --entrypoint");
    }

    let tmp_file = NamedTempFile::new()?;

    {
        let mut zip_file = ZipWriter::new(&tmp_file);
        let options =
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip_file.create_from_directory_with_options(
            &recipe_dir.as_ref().to_owned(),
            |_| options,
            &ZipIgnoreEntryHandler::new(),
        )?;
    }

    Ok(tmp_file.into_temp_path())
}

async fn publish_recipe<P: AsRef<Path>>(
    client: &AdaptiveClient,
    project: &str,
    name: Option<String>,
    key: Option<String>,
    recipe: P,
    entrypoint: Option<String>,
    entrypoint_config: Option<String>,
    force: bool,
) -> Result<()> {
    let name = name.unwrap_or_else(|| {
        recipe
            .as_ref()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    });
    let key = key.unwrap_or_else(|| slugify(&name));

    let existing = client.get_recipe(project.to_string(), key.clone()).await?;

    let (id, key) = if let Some(existing_recipe) = existing {
        if !force {
            bail!(
                "A recipe with key '{}' already exists. Use --force to update it.",
                key
            );
        }

        let recipe_path: Box<dyn AsRef<Path> + Send> = if recipe.as_ref().is_dir() {
            Box::new(zip_recipe_dir(&recipe, &entrypoint)?)
        } else {
            Box::new(recipe.as_ref().to_path_buf())
        };

        let response = client
            .update_recipe(
                project,
                &existing_recipe.id.to_string(),
                Some(name),
                None,
                None,
                Some(recipe_path.as_ref()),
                entrypoint,
                entrypoint_config,
            )
            .await?;

        (response.id, response.key)
    } else {
        let response = if recipe.as_ref().is_dir() {
            let recipe = zip_recipe_dir(recipe, &entrypoint)?;
            client
                .publish_recipe(project, &name, &key, &recipe, entrypoint, entrypoint_config)
                .await?
        } else {
            client
                .publish_recipe(project, &name, &key, recipe, entrypoint, entrypoint_config)
                .await?
        };
        (response.id, response.key)
    };

    if io::stdout().is_terminal() {
        println!(
            "Recipe published successfully with ID: {}, key: {}",
            id,
            key.unwrap_or("<none>".to_string())
        );
    } else {
        println!("{}", id);
    };

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
        .block_on(client.list_recipes(&config.default_project.expect("No default project set")))
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

fn project_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let mut completions = vec![];
    let Some(current) = current.to_str() else {
        return completions;
    };

    let config = config::read_config().expect("Failed to read config");

    let client = AdaptiveClient::new(config.adaptive_base_url, config.adaptive_api_key);

    let handle = Handle::current();
    let projects = handle.block_on(client.list_projects()).unwrap();

    projects.into_iter().for_each(|project| {
        if project.key.starts_with(current) {
            completions.push(CompletionCandidate::new(project.key));
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
    project: &str,
    recipe: String,
    args: Vec<String>,
) -> Result<Map<String, Value>> {
    let recipe_contents = client
        .get_recipe(project.to_string(), recipe.clone())
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

async fn run_recipe(client: &AdaptiveClient, project: &str, run_args: RunArgs) -> Result<()> {
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
        parse_recipe_args(client, project, run_args.recipe.clone(), run_args.args).await?
    };

    let response = client
        .run_recipe(
            project,
            &run_args.recipe.to_string(),
            parameters,
            run_args.name,
            run_args.compute_pool,
            run_args.gpus.unwrap_or(1),
            false,
        )
        .await?;

    if io::stdout().is_terminal() {
        println!("Recipe run successfully with ID: {}", response.id);
    } else {
        println!("{}", response.id);
    }

    Ok(())
}

async fn create_team(client: &AdaptiveClient, name: &str, key: Option<&str>) -> Result<()> {
    let response = client.create_team(name, key).await?;

    if io::stdout().is_terminal() {
        println!(
            "Team created successfully with ID: {}, key: {}",
            response.id, response.key
        );
    } else {
        println!("{}", response.id);
    }

    Ok(())
}

async fn add_team_member(
    client: &AdaptiveClient,
    user: &str,
    team: &str,
    role: &str,
) -> Result<()> {
    let response = client.add_team_member(user, team, role).await?;

    if io::stdout().is_terminal() {
        println!(
            "User {} ({}) added to team {} ({}) with role {}",
            response.user.name,
            response.user.email,
            response.team.name,
            response.team.key,
            response.role.name
        );
    } else {
        println!("{}", response.user.id);
    }

    Ok(())
}

async fn remove_team_member(client: &AdaptiveClient, user: &str, team: &str) -> Result<()> {
    let response = client.remove_team_member(user, team).await?;

    if io::stdout().is_terminal() {
        println!(
            "User {} ({}) removed from team {}",
            response.name, response.email, team
        );
    } else {
        println!("{}", response.id);
    }

    Ok(())
}

async fn list_teams(client: &AdaptiveClient) -> Result<()> {
    let teams = client.list_teams().await?;

    for team in teams {
        println!("{}\t{}\t{}", team.id, team.key, team.name);
    }

    Ok(())
}

async fn list_users(client: &AdaptiveClient) -> Result<()> {
    let users = client.list_users().await?;

    let config = ListConfig {
        columns: vec![
            Column {
                header: "Id",
                width: Some(37),
            },
            Column {
                header: "Email",
                width: Some(30),
            },
            Column {
                header: "Name",
                width: Some(30),
            },
            Column {
                header: "Teams",
                width: None,
            },
        ],
        empty_message: "No users found",
    };
    let rows: Vec<Vec<Cell>> = users
        .iter()
        .map(|user| {
            vec![
                Cell::from(user.id.to_string()),
                Cell::from(user.email.as_str()),
                Cell::from(user.name.as_str()),
                Cell::from(
                    user.teams
                        .iter()
                        .map(|t| format!("{} ({})", t.team.name, t.role.name))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            ]
        })
        .collect();
    let mut el: AnyElement<'static> = render_list(config, rows).into();
    el.print();

    Ok(())
}

async fn describe_user(client: &AdaptiveClient, id_or_email: &str) -> Result<()> {
    let users = client.list_users().await?;

    let user = if let Ok(uuid) = id_or_email.parse::<Uuid>() {
        users.into_iter().find(|u| u.id == uuid)
    } else {
        users.into_iter().find(|u| u.email == id_or_email)
    };

    match user {
        Some(user) => {
            println!("ID:        {}", user.id);
            println!("Email:     {}", user.email);
            println!("Name:      {}", user.name);
            println!("Type:      {:?}", user.user_type);
            println!(
                "Created:   {}",
                humantime::format_rfc3339(user.created_at.0)
            );
            if user.teams.is_empty() {
                println!("Teams:     (none)");
            } else {
                for (i, membership) in user.teams.iter().enumerate() {
                    let label = if i == 0 { "Teams:" } else { "      " };
                    println!(
                        "{}     {} ({})",
                        label, membership.team.name, membership.role.name
                    );
                }
            }
            Ok(())
        }
        None => bail!("User not found: {}", id_or_email),
    }
}

async fn create_user(
    client: &AdaptiveClient,
    name: &str,
    email: Option<EmailAddress>,
    user_type: UserTypeArg,
) -> Result<()> {
    let email = email.map(|e| e.to_string());
    let response = client
        .create_user(name, email.as_deref(), vec![], Some(user_type.into()), None)
        .await?;

    if io::stdout().is_terminal() {
        println!(
            "User created successfully with ID: {}, email {}",
            response.user.id, response.user.email
        );
    } else {
        println!("{}", response.user.id);
    }

    Ok(())
}

async fn delete_user(client: &AdaptiveClient, id_or_email: &str) -> Result<()> {
    let response = client.delete_user(id_or_email).await?;

    if io::stdout().is_terminal() {
        println!(
            "User deleted successfully: {} ({})",
            response.name, response.email
        );
    } else {
        println!("{}", response.id);
    }

    Ok(())
}

async fn create_role(
    client: &AdaptiveClient,
    name: &str,
    key: Option<&str>,
    permissions: Vec<String>,
) -> Result<()> {
    let response = client.create_role(name, key, permissions).await?;

    if io::stdout().is_terminal() {
        println!(
            "Role created successfully with ID: {}, key: {}",
            response.id, response.key
        );
    } else {
        println!("{}", response.id);
    }

    Ok(())
}

async fn list_roles(client: &AdaptiveClient) -> Result<()> {
    let roles = client.list_roles().await?;

    let config = ListConfig {
        columns: vec![
            Column {
                header: "Id",
                width: Some(37),
            },
            Column {
                header: "Key",
                width: Some(20),
            },
            Column {
                header: "Name",
                width: None,
            },
        ],
        empty_message: "No roles found",
    };
    let rows: Vec<Vec<Cell>> = roles
        .iter()
        .map(|role| {
            vec![
                Cell::from(role.id.to_string()),
                Cell::from(role.key.as_str()),
                Cell::from(role.name.as_str()),
            ]
        })
        .collect();
    let mut el: AnyElement<'static> = render_list(config, rows).into();
    el.print();

    Ok(())
}

async fn describe_role(client: &AdaptiveClient, id_or_key: &str) -> Result<()> {
    let roles = client.list_roles().await?;

    let role = if let Ok(uuid) = id_or_key.parse::<Uuid>() {
        roles.into_iter().find(|r| r.id == uuid)
    } else {
        roles.into_iter().find(|r| r.key == id_or_key)
    };

    match role {
        Some(role) => {
            println!("ID:          {}", role.id);
            println!("Key:         {}", role.key);
            println!("Name:        {}", role.name);
            println!(
                "Created:     {}",
                humantime::format_rfc3339(role.created_at.0)
            );
            let mut permissions = role.permissions.clone();
            permissions.sort();
            println!("Permissions: {}", permissions.join(", "));
            Ok(())
        }
        None => bail!("Role not found: {}", id_or_key),
    }
}

async fn add_role_permission(
    client: &AdaptiveClient,
    id_or_key: &str,
    permissions: Vec<String>,
) -> Result<()> {
    let roles = client.list_roles().await?;

    let role = if let Ok(uuid) = id_or_key.parse::<Uuid>() {
        roles.into_iter().find(|r| r.id == uuid)
    } else {
        roles.into_iter().find(|r| r.key == id_or_key)
    };

    let role = role.ok_or_else(|| anyhow::anyhow!("Role not found: {}", id_or_key))?;

    let mut current = role.permissions.clone();
    for perm in &permissions {
        if !current.contains(perm) {
            current.push(perm.clone());
        }
    }

    let updated = client.update_role(id_or_key, None, Some(current)).await?;

    if io::stdout().is_terminal() {
        let mut perms = updated.permissions.clone();
        perms.sort();
        println!(
            "Updated permissions for role '{}': {}",
            updated.key,
            perms.join(", ")
        );
    } else {
        println!("{}", updated.id);
    }

    Ok(())
}

async fn remove_role_permission(
    client: &AdaptiveClient,
    id_or_key: &str,
    permissions: Vec<String>,
) -> Result<()> {
    let roles = client.list_roles().await?;

    let role = if let Ok(uuid) = id_or_key.parse::<Uuid>() {
        roles.into_iter().find(|r| r.id == uuid)
    } else {
        roles.into_iter().find(|r| r.key == id_or_key)
    };

    let role = role.ok_or_else(|| anyhow::anyhow!("Role not found: {}", id_or_key))?;

    let mut current: Vec<String> = role.permissions;
    current.retain(|p| !permissions.contains(p));

    let updated = client.update_role(id_or_key, None, Some(current)).await?;

    if io::stdout().is_terminal() {
        let mut perms = updated.permissions.clone();
        perms.sort();
        println!(
            "Updated permissions for role '{}': {}",
            updated.key,
            perms.join(", ")
        );
    } else {
        println!("{}", updated.id);
    }

    Ok(())
}

async fn list_jobs(client: &AdaptiveClient, project: Option<String>) -> Result<()> {
    let response = client.list_jobs(project).await?;

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

    let default_project_str = read_input(
        "Default Use Case",
        None,
        Some("Optional: Set a default project to avoid specifying --project every time"),
    )?;
    let default_project = if default_project_str.is_empty() {
        None
    } else {
        Some(default_project_str)
    };

    config::set_api_key_keyring(adaptive_api_key)?;

    let config_file = config::ConfigFile {
        adaptive_base_url: Some(adaptive_base_url),
        default_project,
    };

    config::write_config(config_file)?;

    element!(SuccessMessage(message: "Configuration complete!".to_string())).print();

    Ok(())
}
