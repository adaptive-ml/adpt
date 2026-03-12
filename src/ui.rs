use std::sync::Arc;
use std::time::Duration;

use adaptive_client_rust::get_custom_recipes::GetCustomRecipesCustomRecipes;
use adaptive_client_rust::get_job::{GetJobJobStages, GetJobJobStagesInfo, JobStatusOutput};
use adaptive_client_rust::list_all_models::{self, ListAllModelsModels};
use adaptive_client_rust::list_jobs::{self, ListJobsJobsNodes};
use adaptive_client_rust::list_models::{self, ListModelsProjectModelServices};
use adaptive_client_rust::{AdaptiveClient, get_job};
use iocraft::prelude::*;
use tokio::sync::watch::Receiver;
use uuid::Uuid;

pub struct Cell {
    pub content: String,
    pub color: Option<Color>,
}

impl From<String> for Cell {
    fn from(s: String) -> Self {
        Cell {
            content: s,
            color: None,
        }
    }
}

impl From<&str> for Cell {
    fn from(s: &str) -> Self {
        Cell {
            content: s.to_string(),
            color: None,
        }
    }
}

pub struct Column {
    pub header: &'static str,
    pub width: Option<u32>,
}

pub struct ListConfig {
    pub columns: Vec<Column>,
    pub empty_message: &'static str,
}

pub fn render_list(config: ListConfig, rows: Vec<Vec<Cell>>) -> impl Into<AnyElement<'static>> {
    let num_columns = config.columns.len();
    let col_widths: Vec<Option<u32>> = config
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let padding =
                if i == 0 { 1u32 } else { 0 } + if i == num_columns - 1 { 1u32 } else { 0 };
            c.width.map(|w| w.max(c.header.len() as u32 + padding))
        })
        .collect();

    let header_cells: Vec<AnyElement<'static>> = config
        .columns
        .iter()
        .enumerate()
        .map(|(i, col)| {
            let is_first = i == 0;
            let is_last = i == num_columns - 1;
            let width = col_widths[i];
            if let Some(w) = width {
                element! {
                    View(
                        padding_left: if is_first { 1u32 } else { 0 },
                        padding_right: if is_last { 1u32 } else { 0 },
                        width: w,
                    ) {
                        Text(content: col.header, weight: Weight::Bold, decoration: TextDecoration::Underline)
                    }
                }
                .into_any()
            } else {
                element! {
                    View(
                        padding_left: if is_first { 1u32 } else { 0 },
                        padding_right: if is_last { 1u32 } else { 0 },
                    ) {
                        Text(content: col.header, weight: Weight::Bold, decoration: TextDecoration::Underline)
                    }
                }
                .into_any()
            }
        })
        .collect();

    let body: Vec<AnyElement<'static>> = if rows.is_empty() {
        vec![
            element! {
                View(padding: 2u32, justify_content: JustifyContent::Center) {
                    Text(content: config.empty_message, color: Color::Grey)
                }
            }
            .into_any(),
        ]
    } else {
        rows.into_iter()
            .enumerate()
            .map(|(i, row)| {
                let cells: Vec<AnyElement<'static>> = row
                    .into_iter()
                    .enumerate()
                    .map(|(j, cell)| {
                        let is_first = j == 0;
                        let is_last = j == num_columns - 1;
                        let col_width = col_widths.get(j).copied().flatten();
                        if let Some(w) = col_width {
                            element! {
                                View(
                                    padding_left: if is_first { 1u32 } else { 0 },
                                    padding_right: if is_last { 1u32 } else { 0 },
                                    width: w,
                                ) {
                                    Text(
                                        content: cell.content,
                                        color: cell.color.unwrap_or(Color::Reset),
                                    )
                                }
                            }
                            .into_any()
                        } else {
                            element! {
                                View(
                                    padding_left: if is_first { 1u32 } else { 0 },
                                    padding_right: if is_last { 1u32 } else { 0 },
                                ) {
                                    Text(
                                        content: cell.content,
                                        color: cell.color.unwrap_or(Color::Reset),
                                    )
                                }
                            }
                            .into_any()
                        }
                    })
                    .collect();

                element! {
                    View(
                        background_color: if i % 2 == 0 { None } else { Some(Color::Grey) },
                        gap: 2u32,
                    ) {
                        #(cells)
                    }
                }
                .into_any()
            })
            .collect()
    };

    element! {
        View(
            flex_direction: FlexDirection::Column,
            border_style: BorderStyle::Round,
            border_color: Color::Cyan,
        ) {
            View(
                border_style: BorderStyle::Single,
                border_edges: Edges::Bottom,
                border_color: Color::Grey,
                gap: 2u32,
            ) {
                #(header_cells)
            }
            #(body)
        }
    }
}

#[derive(Default, Props)]
pub struct ProgressBarProps {
    pub title: String,
    pub progress: Option<Receiver<f32>>,
}

#[component]
pub fn ProgressBar(mut hooks: Hooks, props: &ProgressBarProps) -> impl Into<AnyElement<'static>> {
    let mut progress = hooks.use_state::<f32, _>(|| 0.0);
    let mut recv = props.progress.clone().unwrap();

    hooks.use_future(async move {
        loop {
            if recv.changed().await.is_ok() {
                let new_value = *recv.borrow();
                progress.set(new_value);
            } else {
                break;
            }
        }
    });

    element! {
        View {
            Text(content: props.title.clone())
            View(margin_left: 1, margin_right: 1, border_style: BorderStyle::Single, border_edges: Edges::Left | Edges::Right, border_color: Color::Blue, width: 60) {
                View(width: Percent(progress.get()), height: 1, background_color: Color::Green)
            }
            View() {
                Text(content: format!("{:.0}%", progress))
            }
        }
    }
}

#[derive(Default, Props)]
pub struct RecipeListProps {
    pub recipes: Vec<GetCustomRecipesCustomRecipes>,
}

#[component]
pub fn RecipeList(props: &RecipeListProps) -> impl Into<AnyElement<'static>> {
    let config = ListConfig {
        columns: vec![Column {
            header: "Name",
            width: None,
        }],
        empty_message: "No recipes found",
    };
    let rows: Vec<Vec<Cell>> = props
        .recipes
        .iter()
        .map(|recipe| vec![Cell::from(recipe.name.as_str())])
        .collect();
    render_list(config, rows)
}

#[derive(Default, Props)]
pub struct JobsListProps {
    pub jobs: Vec<ListJobsJobsNodes>,
}

fn job_status_cell(status: &list_jobs::JobStatus) -> Cell {
    match status {
        list_jobs::JobStatus::PENDING => Cell {
            content: "⏳".to_string(),
            color: Some(Color::Yellow),
        },
        list_jobs::JobStatus::RUNNING => Cell {
            content: "▶️".to_string(),
            color: Some(Color::Yellow),
        },
        list_jobs::JobStatus::COMPLETED => Cell {
            content: "✅".to_string(),
            color: Some(Color::Green),
        },
        list_jobs::JobStatus::FAILED => Cell {
            content: "❌".to_string(),
            color: Some(Color::Red),
        },
        list_jobs::JobStatus::CANCELED => Cell {
            content: "🚫".to_string(),
            color: Some(Color::Yellow),
        },
        list_jobs::JobStatus::Other(_) => Cell {
            content: "❓".to_string(),
            color: Some(Color::Yellow),
        },
    }
}

trait ModelDisplay {
    fn get_status(&self) -> String;
    fn get_id(&self) -> String;
    fn get_name(&self) -> &str;
    fn get_key(&self) -> &str;
}

impl ModelDisplay for ListModelsProjectModelServices {
    fn get_status(&self) -> String {
        match self.status {
            list_models::ModelServiceStatus::PENDING => "Pending".to_string(),
            list_models::ModelServiceStatus::ONLINE => "Online".to_string(),
            list_models::ModelServiceStatus::OFFLINE => "Offline".to_string(),
            list_models::ModelServiceStatus::TURNED_OFF => "Turned Off".to_string(),
            list_models::ModelServiceStatus::ERROR => "Error".to_string(),
            list_models::ModelServiceStatus::Other(ref other) => other.to_owned(),
        }
    }

    fn get_id(&self) -> String {
        self.id.to_string()
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_key(&self) -> &str {
        &self.key
    }
}

impl ModelDisplay for ListAllModelsModels {
    fn get_status(&self) -> String {
        if self.error.is_some() {
            "Error".to_string()
        } else if self.is_training {
            "Training".to_string()
        } else {
            match &self.online {
                list_all_models::ModelOnline::ONLINE => "Online".to_string(),
                list_all_models::ModelOnline::OFFLINE => "Offline".to_string(),
                list_all_models::ModelOnline::PENDING => "Pending".to_string(),
                list_all_models::ModelOnline::ERROR => "Error".to_string(),
                list_all_models::ModelOnline::Other(other) => other.to_owned(),
            }
        }
    }

    fn get_id(&self) -> String {
        self.id.to_string()
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_key(&self) -> &str {
        if self.key.is_empty() {
            "N/A"
        } else {
            &self.key
        }
    }
}

fn models_list_config() -> ListConfig {
    ListConfig {
        columns: vec![
            Column {
                header: "Status",
                width: None,
            },
            Column {
                header: "Id",
                width: Some(36),
            },
            Column {
                header: "Name",
                width: Some(25),
            },
            Column {
                header: "Key",
                width: None,
            },
        ],
        empty_message: "No models found",
    }
}

fn model_to_row(model: &dyn ModelDisplay) -> Vec<Cell> {
    vec![
        Cell::from(model.get_status()),
        Cell::from(model.get_id()),
        Cell::from(model.get_name()),
        Cell::from(model.get_key()),
    ]
}

#[derive(Default, Props)]
pub struct ModelsListProps {
    pub model_services: Vec<ListModelsProjectModelServices>,
}

#[component]
pub fn ModelsList(props: &ModelsListProps) -> impl Into<AnyElement<'static>> {
    let config = models_list_config();
    let rows: Vec<Vec<Cell>> = props
        .model_services
        .iter()
        .map(|m| model_to_row(m))
        .collect();
    render_list(config, rows)
}

#[derive(Default, Props)]
pub struct AllModelsListProps {
    pub models: Vec<ListAllModelsModels>,
}

#[component]
pub fn AllModelsList(props: &AllModelsListProps) -> impl Into<AnyElement<'static>> {
    let config = models_list_config();
    let rows: Vec<Vec<Cell>> = props.models.iter().map(|m| model_to_row(m)).collect();
    render_list(config, rows)
}

#[component]
pub fn JobsList(props: &JobsListProps) -> impl Into<AnyElement<'static>> {
    let config = ListConfig {
        columns: vec![
            Column {
                header: "Status",
                width: Some(6),
            },
            Column {
                header: "Id",
                width: Some(36),
            },
            Column {
                header: "Duration",
                width: Some(8),
            },
            Column {
                header: "User",
                width: None,
            },
        ],
        empty_message: "No jobs found",
    };
    let mut sorted = props.jobs.clone();
    sorted.sort_by(|job1, job2| job1.created_at.cmp(&job2.created_at).reverse());
    let rows: Vec<Vec<Cell>> = sorted
        .iter()
        .map(|job| {
            vec![
                job_status_cell(&job.status),
                Cell::from(job.id.to_string()),
                Cell::from(
                    humantime::format_duration(Duration::from_millis(
                        job.duration_ms.unwrap_or_default() as u64,
                    ))
                    .to_string(),
                ),
                Cell::from(
                    job.created_by
                        .as_ref()
                        .map(|user| format!("{} <{}>", user.name, user.email))
                        .unwrap_or("Unknown".to_string()),
                ),
            ]
        })
        .collect();
    render_list(config, rows)
}

#[derive(Default, Props)]
pub struct JobStatusProps {
    pub name: String,
    pub stages: Vec<GetJobJobStages>,
    pub status: String,
    pub error: Option<String>,
}

struct CommonJobFields {
    processed_num_samples: Option<i64>,
    total_num_samples: Option<i64>,
}

fn get_common_stage_info(stage: &GetJobJobStagesInfo) -> CommonJobFields {
    match stage {
        GetJobJobStagesInfo::TrainingJobStageOutput(training) => CommonJobFields {
            processed_num_samples: training.processed_num_samples,
            total_num_samples: training.total_num_samples,
        },
        GetJobJobStagesInfo::EvalJobStageOutput(eval) => CommonJobFields {
            processed_num_samples: eval.processed_num_samples,
            total_num_samples: eval.total_num_samples,
        },
        GetJobJobStagesInfo::BatchInferenceJobStageOutput(batch_inference) => CommonJobFields {
            processed_num_samples: batch_inference.processed_num_samples,
            total_num_samples: batch_inference.total_num_samples,
        },
    }
}

#[derive(Default, Props)]
pub struct FollowJobStatusProps {
    pub client: Option<Arc<AdaptiveClient>>,
    pub job_id: Uuid,
}

#[component]
pub fn FollowJobStatus(
    props: &FollowJobStatusProps,
    mut hooks: Hooks,
) -> impl Into<AnyElement<'static>> {
    let mut system = hooks.use_context_mut::<SystemContext>();
    let mut stages = hooks.use_state(Vec::new);
    let mut status = hooks.use_state(|| get_job::JobStatus::PENDING);
    let mut name = hooks.use_state(String::new);
    let mut error = hooks.use_state(|| None);
    let mut should_exit = hooks.use_state(|| false);
    let client = props.client.clone().unwrap();
    let job_id = props.job_id;

    hooks.use_future(async move {
        loop {
            let job = client.get_job(job_id).await.unwrap();

            stages.set(job.stages);
            status.set(job.status.clone());
            name.set(job.name);
            error.set(job.error);

            let is_running = matches!(
                job.status,
                get_job::JobStatus::PENDING | get_job::JobStatus::RUNNING
            );

            if !is_running {
                should_exit.set(true);
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    });

    if should_exit.get() {
        system.exit();
    }

    let stages = stages.read().clone();
    let status = status.read().clone().to_string();
    let name = name.read().clone();
    let error = error.read().clone();

    element! {
        JobStatus (
            name: name,
            stages: stages,
            status: status,
            error: error
        )
    }
}

#[derive(Default, Props)]
struct StatusIconProps {
    status: Option<JobStatusOutput>,
}

#[component]
fn StatusIcon(props: &StatusIconProps) -> impl Into<AnyElement<'static>> {
    match props.status {
        Some(JobStatusOutput::PENDING) => element! {
            Text (
                color: Color::Reset,
                content: "◇"
            )
        }
        .into_any(),
        Some(JobStatusOutput::RUNNING) => element! {
            Spinner()
        }
        .into_any(),
        Some(JobStatusOutput::DONE) => element! {
            Text (
                color: Color::Green,
                content: "◆"
            )
        }
        .into_any(),
        Some(JobStatusOutput::CANCELLED) => element! {
            Text (
                color: Color::Red,
                content: "■"
            )
        }
        .into_any(),
        Some(JobStatusOutput::ERROR) => element! {
            Text (
                color: Color::Red,
                content: "▲"
            )
        }
        .into_any(),
        _ => element! {
            Text (
                color: Color::Yellow,
                content: "❓"
            )
        }
        .into_any(),
    }
}

#[derive(Default, Props)]
struct JobStageProps {
    stage: Option<GetJobJobStages>,
}

#[component]
fn JobStage(props: &JobStageProps) -> impl Into<AnyElement<'static>> {
    let stage = props.stage.as_ref().unwrap();
    let info = stage.info.as_ref().map(get_common_stage_info);
    if let Some(info) = info {
        let progress = if let (Some(processed), Some(total)) =
            (info.processed_num_samples, info.total_num_samples)
        {
            format!("{}/{}", processed, total)
        } else {
            "Unknown".to_owned()
        };
        element! {
            View(flex_direction: FlexDirection::Column) {
                Text(content: "│")
                View(flex_direction: FlexDirection::Row) {
                    StatusIcon(status: stage.status.clone())
                    Text(weight: Weight::Bold, content: format!(" {}", &stage.name))
                }
                Text(content: format!("│ {}", progress))
            }
        }
    } else {
        element! {
            View(flex_direction: FlexDirection::Column) {
                Text(content: "│")
                View(flex_direction: FlexDirection::Row) {
                    StatusIcon(status: stage.status.clone())
                    Text(weight: Weight::Bold, content: format!(" {}", &stage.name))
                }
            }
        }
    }
}

#[component]
pub fn JobStatus(props: &JobStatusProps) -> impl Into<AnyElement<'static>> {
    element! {
        View(flex_direction: FlexDirection::Column) {
            View(flex_direction: FlexDirection::Row) {
                Text(content: "┌ ")
                View(background_color: Color::Blue) {
                    Text(content: &props.name, color: Color::White)
                }
            }
            #(props.stages.clone().into_iter().map(|stage| {
                element! {
                    JobStage(stage: stage)
                }
            }))
            Text(content: "│")
            View(flex_direction: FlexDirection::Row) {
                Text(content: "└ ")
                Text(content: &props.status)
                #(props.error.as_ref().map(|error| element! {
                    Text(content: format!(": {}", error))
                }))
            }
        }
    }
}

#[derive(Default, Props)]
pub struct SpinnerProps {
    pub color: Option<Color>,
}

#[component]
pub fn Spinner(props: &SpinnerProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut frame = hooks.use_state(|| 0usize);

    hooks.use_future(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
            frame.set((frame.get() + 1) % 4);
        }
    });

    let spinner_chars = ["◐", "◓", "◑", "◒"];
    let current_char = spinner_chars[*frame.read()];
    let color = props.color.unwrap_or(Color::Cyan);

    element! {
        Text(content: current_char, color: color)
    }
}

#[derive(Default, Props)]
pub struct ConfigHeaderProps {}

#[component]
pub fn ConfigHeader(_props: &ConfigHeaderProps) -> impl Into<AnyElement<'static>> {
    element! {
        View(flex_direction: FlexDirection::Column, margin_bottom: 2) {
            Text(
                content: "⚙️  Configure adpt",
                weight: Weight::Bold,
                color: Color::Blue
            )
            Text(
                content: "Set up your Adaptive CLI configuration",
                color: Color::DarkGrey
            )
        }
    }
}

#[derive(Default, Props)]
pub struct InputPromptProps {
    pub prompt: String,
    pub default: Option<String>,
    pub description: Option<String>,
}

#[component]
pub fn InputPrompt(props: &InputPromptProps) -> impl Into<AnyElement<'static>> {
    element! {
        View(flex_direction: FlexDirection::Column, margin_bottom: 1) {
            Text(
                content: format!("{}:", props.prompt),
                weight: Weight::Bold,
                color: Color::Cyan
            )
            #(props.description.as_ref().map(|desc| {
                element! {
                    Text(
                        content: format!("  {}", desc),
                        color: Color::DarkGrey
                    )
                }
            }))
            #(props.default.as_ref().map(|def| {
                element! {
                    Text(
                        content: format!("  [default: {}]", def),
                        color: Color::DarkGrey
                    )
                }
            }))
        }
    }
}

#[derive(Default, Props)]
pub struct ErrorMessageProps {
    pub message: String,
}

#[component]
pub fn ErrorMessage(props: &ErrorMessageProps) -> impl Into<AnyElement<'static>> {
    element! {
        Text(
            content: format!("✗ {}", props.message),
            color: Color::Red
        )
    }
}

#[derive(Default, Props)]
pub struct SuccessMessageProps {
    pub message: String,
}

#[component]
pub fn SuccessMessage(props: &SuccessMessageProps) -> impl Into<AnyElement<'static>> {
    element! {
        View(margin_top: 1) {
            Text(
                content: format!("✓ {}", props.message),
                weight: Weight::Bold,
                color: Color::Green
            )
        }
    }
}
