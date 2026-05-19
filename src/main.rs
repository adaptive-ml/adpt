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
use typed_path::Utf8TypedPath;
use url::Url;
use uuid::Uuid;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use zip_extensions::{
    zip_ignore_entry_handler::ZipIgnoreEntryHandler, zip_writer_extensions::ZipWriterExtensions,
};

use crate::{
    config::DeploymentConfig,
    json_schema::{JsonSchema, JsonSchemaPropertyContents},
    terminal::TitleGuard,
    ui::{
        AllModelsList, Cell, Column, ConfigHeader, ErrorMessage, InputPrompt, JobsList, ListConfig,
        ModelsList, ProgressBar, RecipeList, SuccessMessage, deployment_color, render_list,
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
    /// Use a specific deployment for this invocation, overriding the active one
    #[arg(long, global = true)]
    deployment: Option<String>,
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
enum DeploymentCommands {
    /// Create or edit a deployment interactively
    Setup {
        /// Deployment name. If omitted, edits the active deployment.
        name: Option<String>,
        /// Base URL (for non-interactive setup, e.g. in CI)
        #[arg(long)]
        url: Option<Url>,
        /// Default project (for non-interactive setup)
        #[arg(long)]
        default_project: Option<String>,
        /// API key (for non-interactive setup)
        #[arg(long)]
        api_key: Option<String>,
    },
    /// List all configured deployments
    List,
    /// Show details for a deployment
    Show {
        /// Deployment name. Defaults to the active deployment.
        name: Option<String>,
    },
    /// Pin a shell to a deployment (spawns a subshell with $ADPT_DEPLOYMENT)
    Use {
        /// Deployment name to activate
        name: String,
        /// Also persist as the file-level active deployment for fresh shells
        #[arg(short, long)]
        persist: bool,
    },
    /// Print just the active deployment name (for shell prompts)
    Current,
    /// Remove a deployment and its stored API key
    Remove {
        /// Deployment name to remove
        name: String,
        /// Allow removing the active deployment
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum Commands {
    /// Cancel a job
    Cancel { id: Uuid },
    /// Manage Adaptive deployments
    Deployment {
        #[command(subcommand)]
        command: DeploymentCommands,
    },
    /// Show the active deployment and its resolved settings
    Whoami,
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
        /// Custom entrypoint file
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        entrypoint: Option<String>,
        /// Custom config entrypoint file
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
            Commands::Deployment { .. } => "deployment",
            Commands::Whoami => "whoami",
            Commands::Job { .. } => "job",
            Commands::Jobs => "jobs",
            Commands::Models { .. } => "models",
            Commands::Upload { .. } => "upload",
            Commands::Publish { .. } => "publish",
            Commands::Recipes { .. } => "recipes",
            Commands::Run { .. } => "run",
            Commands::Schema { .. } => "schema",
            Commands::Role { .. } => "role",
            Commands::User { .. } => "user",
            Commands::Team { .. } => "team",
        }
    }
}

fn build_adaptive_client(base_url: Url, api_key: String) -> AdaptiveClient {
    let inner = reqwest::Client::builder()
        .user_agent(concat!("adpt/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("Failed to build HTTP client");
    let http_client = reqwest_middleware::ClientBuilder::new(inner).build();
    AdaptiveClient::new(http_client, base_url, api_key, None)
}

fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

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

    let deployment_override = cli.deployment.clone();
    rt.block_on(async {
        match cli.command {
            Commands::Deployment { command } => {
                handle_deployment_command(command, deployment_override.as_deref())
            }
            Commands::Whoami => print_whoami(deployment_override.as_deref()),
            requires_api_key => {
                ensure_deployment_configured()?;
                let config = config::read_config(deployment_override.as_deref())?;
                let client = build_adaptive_client(config.adaptive_base_url, config.adaptive_api_key);
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
                    Commands::Deployment { .. } => panic!("This state should be unreachable"),
                    Commands::Whoami => panic!("This state should be unreachable"),
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

fn zip_recipe_dir<P: AsRef<Path>>(
    recipe_dir: P,
    entrypoint: &Option<String>,
    entrypoint_config: &Option<String>,
) -> Result<TempPath> {
    if let Some(ep) = entrypoint {
        if !recipe_dir.as_ref().join(ep).is_file() {
            bail!("Entrypoint file '{ep}' does not exist in recipe directory");
        }
    } else if !recipe_dir.as_ref().join("main.py").is_file() {
        bail!("Recipe directory must contain a main.py file, or specify --entrypoint");
    }

    if let Some(ep) = entrypoint_config
        && !recipe_dir.as_ref().join(ep).is_file()
    {
        bail!("Config entrypoint file '{ep}' does not exist in recipe directory");
    }

    let tmp_file = NamedTempFile::new()?;

    {
        let mut zip_file = ZipWriter::new(&tmp_file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip_file.create_from_directory_with_options(
            &recipe_dir.as_ref().to_owned(),
            |_| options,
            &ZipIgnoreEntryHandler::new(),
        )?;
    }

    Ok(tmp_file.into_temp_path())
}

fn resolve_entrypoint(recipe_dir: &Path, entrypoint: Option<String>) -> Result<Option<String>> {
    let Some(ep) = entrypoint else {
        return Ok(None);
    };

    let ep_path = Path::new(&ep);
    if ep_path.extension().and_then(|e| e.to_str()) != Some("py") {
        bail!("entrypoint must be a Python file (.py)");
    }

    let relative = ep_path.strip_prefix(recipe_dir).unwrap_or(ep_path);

    let resolved = recipe_dir.join(relative);
    if !resolved.starts_with(recipe_dir) {
        bail!("entrypoint must be contained within the recipe directory");
    }

    let unix = Utf8TypedPath::derive(&relative.to_string_lossy()).with_unix_encoding();
    Ok(Some(unix.to_string()))
}

#[allow(clippy::too_many_arguments)]
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
    let entrypoint = resolve_entrypoint(recipe.as_ref(), entrypoint)?;
    let entrypoint_config = resolve_entrypoint(recipe.as_ref(), entrypoint_config)?;

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
            Box::new(zip_recipe_dir(&recipe, &entrypoint, &entrypoint_config)?)
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
            let recipe = zip_recipe_dir(recipe, &entrypoint, &entrypoint_config)?;
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

    let Ok(config) = config::read_config(None) else {
        return completions;
    };
    let Some(default_project) = config.default_project else {
        return completions;
    };

    let client = build_adaptive_client(config.adaptive_base_url, config.adaptive_api_key);

    let handle = Handle::current();
    let Ok(recipes) = handle.block_on(client.list_recipes(&default_project)) else {
        return completions;
    };

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

    let Ok(config) = config::read_config(None) else {
        return completions;
    };

    let client = build_adaptive_client(config.adaptive_base_url, config.adaptive_api_key);

    let handle = Handle::current();
    let Ok(projects) = handle.block_on(client.list_projects()) else {
        return completions;
    };

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

    let Ok(config) = config::read_config(None) else {
        return completions;
    };

    let client = build_adaptive_client(config.adaptive_base_url, config.adaptive_api_key);

    let handle = Handle::current();
    let Ok(pools) = handle.block_on(client.list_pools()) else {
        return completions;
    };

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

/// Prompt for a deployment name when none was provided. Returns the resolved name.
fn resolve_setup_target(provided: Option<String>) -> Result<(String, Option<DeploymentConfig>)> {
    let file = config::read_config_file()?;
    let raw = if let Some(name) = provided {
        name
    } else if let Some(active) = file.active_deployment.clone() {
        active
    } else {
        let entered = read_input(
            "Deployment name",
            Some("default"),
            Some("A short label for this Adaptive deployment (e.g. prod, staging)"),
        )?;
        if entered.is_empty() {
            bail!("Deployment name cannot be empty");
        }
        entered
    };
    let name = config::normalize_name(&raw);
    if name.is_empty() {
        bail!("Deployment name must contain at least one alphanumeric character");
    }
    if name != raw {
        element! {
            Text(
                content: format!("  Using `{name}` as the deployment name."),
                color: Color::DarkGrey,
            )
        }
        .print();
    }
    let existing = file.deployments.get(&name).cloned();
    Ok((name, existing))
}

fn deployment_setup(
    name: Option<String>,
    url: Option<Url>,
    default_project: Option<String>,
    api_key: Option<String>,
) -> Result<()> {
    let (name, existing) = resolve_setup_target(name)?;
    let is_edit = existing.is_some();

    let title = if is_edit {
        format!("⚙️  Edit deployment: {name}")
    } else {
        format!("⚙️  Set up deployment: {name}")
    };
    element!(ConfigHeader(title: Some(title), subtitle: None)).print();

    let interactive = url.is_none() && api_key.is_none() && io::stdout().is_terminal();

    // Resolve base URL
    let adaptive_base_url = if let Some(u) = url {
        u
    } else if interactive {
        let default = existing
            .as_ref()
            .map(|d| d.adaptive_base_url.to_string())
            .unwrap_or_else(|| DEFAULT_ADAPTIVE_BASE_URL.to_string());
        loop {
            let entered = read_input(
                "Adaptive Base URL",
                Some(&default),
                Some("The base URL for your Adaptive instance"),
            )?;
            match Url::parse(&entered) {
                Ok(url) => break url,
                Err(e) => {
                    element!(ErrorMessage(message: format!("Invalid URL: {}", e))).print();
                    println!();
                }
            }
        }
    } else if let Some(d) = &existing {
        d.adaptive_base_url.clone()
    } else {
        bail!("--url is required when running non-interactively for a new deployment");
    };

    // Resolve API key
    let api_key_value: Option<String> = if let Some(k) = api_key {
        Some(k)
    } else if interactive {
        let key_existed = config::get_api_key(&name)?.is_some();
        let description = if key_existed {
            "Your Adaptive API key (leave blank to keep the existing one)"
        } else {
            "Your Adaptive API key (stored securely in OS keyring)"
        };
        let entered = read_input("API Key", None, Some(description))?;
        if entered.is_empty() {
            if !key_existed && !is_edit {
                element!(ErrorMessage(
                    message: "API key required for a new deployment".to_string()
                ))
                .print();
                bail!("Aborted");
            }
            None
        } else {
            Some(entered)
        }
    } else {
        None
    };

    // Resolve default project
    let default_project_value: Option<String> = if let Some(p) = default_project {
        if p.is_empty() { None } else { Some(p) }
    } else if interactive {
        let default = existing.as_ref().and_then(|d| d.default_project.clone());
        let entered = read_input(
            "Default project",
            default.as_deref(),
            Some("Optional: avoids needing --project on every command"),
        )?;
        if entered.is_empty() {
            None
        } else {
            Some(entered)
        }
    } else {
        existing.as_ref().and_then(|d| d.default_project.clone())
    };

    config::upsert_deployment(
        &name,
        DeploymentConfig {
            adaptive_base_url,
            default_project: default_project_value,
        },
    )?;
    if let Some(key) = api_key_value {
        config::set_api_key(&name, &key)?;
    }

    let action = if is_edit { "updated" } else { "saved" };
    element!(SuccessMessage(
        message: format!("Deployment `{name}` {action}.")
    ))
    .print();

    // Drop the user into a pinned shell so they can immediately use the deployment they just set up.
    spawn_pinned_shell(&name)
}

fn deployment_use(name: String, persist: bool) -> Result<()> {
    let name = config::normalize_name(&name);
    let file = config::read_config_file()?;
    if !file.deployments.contains_key(&name) {
        bail!("Deployment `{name}` is not configured. Run `adpt deployment setup {name}`.");
    }
    if persist {
        config::set_active(&name)?;
    }
    spawn_pinned_shell(&name)
}

/// Spawn a new interactive shell with `$ADPT_DEPLOYMENT=<name>` exported, so
/// every command in that shell resolves to this deployment. If the current
/// process is already such a pinned shell (i.e. `$ADPT_DEPLOYMENT` is set),
/// we re-exec the parent shell instead of nesting.
///
/// In non-TTY contexts we emit `export ADPT_DEPLOYMENT=<name>` on stdout so
/// the caller can `eval $(adpt deployment use <name>)` in a script.
fn spawn_pinned_shell(name: &str) -> Result<()> {
    if !io::stdout().is_terminal() || std::env::var("ADPT_NO_SUBSHELL").is_ok() {
        println!("export ADPT_DEPLOYMENT={}", shell_escape(name));
        return Ok(());
    }

    #[cfg(unix)]
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    #[cfg(windows)]
    let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());

    element!(SuccessMessage(
        message: format!("Pinned shell to `{name}` (exit to leave).")
    ))
    .print();

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(&shell)
            .env("ADPT_DEPLOYMENT", name)
            .exec();
        // exec only returns on failure.
        Err(anyhow!("Failed to exec `{shell}`: {err}"))
    }

    #[cfg(windows)]
    {
        let status = std::process::Command::new(&shell)
            .env("ADPT_DEPLOYMENT", name)
            .status()
            .map_err(|e| anyhow!("Failed to spawn `{shell}`: {e}"))?;
        std::process::exit(status.code().unwrap_or(0));
    }
}

fn shell_escape(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

fn handle_deployment_command(
    command: DeploymentCommands,
    deployment_override: Option<&str>,
) -> Result<()> {
    match command {
        DeploymentCommands::Setup {
            name,
            url,
            default_project,
            api_key,
        } => deployment_setup(name, url, default_project, api_key),
        DeploymentCommands::List => deployment_list(),
        DeploymentCommands::Show { name } => deployment_show(name.as_deref(), deployment_override),
        DeploymentCommands::Use { name, persist } => deployment_use(name, persist),
        DeploymentCommands::Current => deployment_current(deployment_override),
        DeploymentCommands::Remove { name, force } => {
            config::remove_deployment(&name, force)?;
            element!(SuccessMessage(
                message: format!("Deployment `{name}` removed.")
            ))
            .print();
            Ok(())
        }
    }
}

fn deployment_list() -> Result<()> {
    let file = config::read_config_file()?;
    if file.deployments.is_empty() {
        println!("No deployments configured. Run `adpt deployment setup <name>`.");
        return Ok(());
    }
    let active = file.active_deployment.as_deref();
    let is_tty = io::stdout().is_terminal();
    for (name, cfg) in &file.deployments {
        let marker = if active == Some(name.as_str()) {
            "*"
        } else {
            " "
        };
        if is_tty {
            element! {
                View(flex_direction: FlexDirection::Row) {
                    Text(content: format!("{marker} "))
                    Text(content: name.clone(), color: deployment_color(name), weight: Weight::Bold)
                    Text(content: format!("  {}", cfg.adaptive_base_url), color: Color::DarkGrey)
                }
            }
            .print();
        } else {
            println!("{marker} {name}\t{}", cfg.adaptive_base_url);
        }
    }
    Ok(())
}

fn deployment_show(name: Option<&str>, deployment_override: Option<&str>) -> Result<()> {
    let file = config::read_config_file()?;
    let target = name
        .map(config::normalize_name)
        .or_else(|| deployment_override.map(config::normalize_name))
        .or_else(|| file.active_deployment.clone())
        .ok_or_else(|| anyhow!("No deployment specified and none active."))?;
    let cfg = file
        .deployments
        .get(&target)
        .ok_or_else(|| anyhow!("Deployment `{target}` is not configured."))?;
    let key_present = config::get_api_key(&target)?.is_some();
    let is_tty = io::stdout().is_terminal();
    if is_tty {
        element! {
            View(flex_direction: FlexDirection::Column) {
                View(flex_direction: FlexDirection::Row) {
                    Text(content: "Deployment: ", weight: Weight::Bold)
                    Text(content: target.clone(), color: deployment_color(&target), weight: Weight::Bold)
                }
                Text(content: format!("URL:             {}", cfg.adaptive_base_url))
                Text(content: format!(
                    "Default project: {}",
                    cfg.default_project.clone().unwrap_or_else(|| "<none>".to_string())
                ))
                Text(content: format!("API key:         {}", if key_present { "set" } else { "not set" }))
            }
        }
        .print();
    } else {
        println!("name\t{target}");
        println!("url\t{}", cfg.adaptive_base_url);
        println!(
            "default_project\t{}",
            cfg.default_project.clone().unwrap_or_default()
        );
        println!("api_key\t{}", if key_present { "set" } else { "unset" });
    }
    Ok(())
}

fn deployment_current(deployment_override: Option<&str>) -> Result<()> {
    let file = config::read_config_file()?;
    let env_pinned = std::env::var("ADPT_DEPLOYMENT")
        .ok()
        .filter(|s| !s.is_empty());
    let name = if let Some(o) = deployment_override {
        config::normalize_name(o)
    } else if let Some(env_name) = env_pinned {
        config::normalize_name(&env_name)
    } else if let Some(a) = file.active_deployment.clone() {
        a
    } else if file.deployments.len() == 1 {
        file.deployments.keys().next().unwrap().clone()
    } else {
        // Empty output (no trailing newline) is more shell-prompt friendly.
        return Ok(());
    };
    if io::stdout().is_terminal() {
        element! {
            Text(content: name.clone(), color: deployment_color(&name), weight: Weight::Bold)
        }
        .print();
    } else {
        // No ANSI when piped — prompts that wrap this command will add their own styling.
        print!("{name}");
        io::stdout().flush()?;
    }
    Ok(())
}

fn print_whoami(deployment_override: Option<&str>) -> Result<()> {
    let config = config::read_config(deployment_override)?;
    // read_config invoked dotenv, so env::var sees the merged environment.
    let url_env = std::env::var("ADAPTIVE_BASE_URL").ok();
    let key_env = std::env::var("ADAPTIVE_API_KEY").ok();
    let project_env = std::env::var("DEFAULT_PROJECT").ok();

    let deployment_env = std::env::var("ADPT_DEPLOYMENT")
        .ok()
        .filter(|s| !s.is_empty());
    let deployment_note = if deployment_override.is_some() {
        Some("via --deployment".to_string())
    } else {
        deployment_env
            .as_ref()
            .map(|_| "via $ADPT_DEPLOYMENT".to_string())
    };
    let url_note = url_env
        .as_ref()
        .map(|_| "via ADAPTIVE_BASE_URL".to_string());
    let key_note = key_env.as_ref().map(|_| "via ADAPTIVE_API_KEY".to_string());
    let project_note = project_env
        .as_ref()
        .map(|_| "via DEFAULT_PROJECT".to_string());

    let is_tty = io::stdout().is_terminal();
    let project_value = config
        .default_project
        .clone()
        .unwrap_or_else(|| "<none>".to_string());

    let fmt_note = |note: Option<&String>| -> String {
        note.map(|n| format!("  [{}]", n)).unwrap_or_default()
    };

    let deployment_overridden = deployment_note.is_some();
    let fields_overridden = url_note.is_some() || key_note.is_some() || project_note.is_some();
    let name_color = if deployment_overridden || fields_overridden {
        Color::Yellow
    } else {
        deployment_color(&config.deployment_name)
    };

    if is_tty {
        element! {
            View(flex_direction: FlexDirection::Column) {
                View(flex_direction: FlexDirection::Row) {
                    Text(content: "Active deployment: ", weight: Weight::Bold)
                    Text(
                        content: config.deployment_name.clone(),
                        color: name_color,
                        weight: Weight::Bold,
                    )
                    Text(content: fmt_note(deployment_note.as_ref()), color: Color::Yellow)
                }
                View(flex_direction: FlexDirection::Row) {
                    Text(content: format!("URL:               {}", config.adaptive_base_url))
                    Text(content: fmt_note(url_note.as_ref()), color: Color::Yellow)
                }
                View(flex_direction: FlexDirection::Row) {
                    Text(content: format!("Default project:   {}", project_value))
                    Text(content: fmt_note(project_note.as_ref()), color: Color::Yellow)
                }
                View(flex_direction: FlexDirection::Row) {
                    Text(content: "API key:           set".to_string())
                    Text(content: fmt_note(key_note.as_ref()), color: Color::Yellow)
                }
            }
        }
        .print();
    } else {
        let suffix = |n: Option<&String>| n.map(|s| format!("\t{}", s)).unwrap_or_default();
        println!(
            "deployment\t{}{}",
            config.deployment_name,
            suffix(deployment_note.as_ref())
        );
        println!(
            "url\t{}{}",
            config.adaptive_base_url,
            suffix(url_note.as_ref())
        );
        println!(
            "default_project\t{}{}",
            config.default_project.unwrap_or_default(),
            suffix(project_note.as_ref())
        );
        if let Some(note) = key_note {
            println!("api_key\tset\t{}", note);
        }
    }
    Ok(())
}

/// If no deployment is configured, prompt the user to set one up (TTY) or
/// surface a clear error (non-TTY) before any command that needs config runs.
fn ensure_deployment_configured() -> Result<()> {
    let file = config::read_config_file()?;
    if !file.deployments.is_empty() {
        return Ok(());
    }
    if io::stdout().is_terminal() && io::stdin().is_terminal() {
        element!(ErrorMessage(
            message: "No deployment configured.".to_string()
        ))
        .print();
        let answer = read_input("Set one up now?", Some("Y"), Some("[Y/n]"))?;
        if !answer.is_empty()
            && !answer.eq_ignore_ascii_case("y")
            && !answer.eq_ignore_ascii_case("yes")
        {
            bail!("Aborted. Run `adpt deployment setup <name>` to configure.");
        }
        deployment_setup(None, None, None, None)
    } else {
        bail!("No deployment configured. Run `adpt deployment setup <name>`.");
    }
}
