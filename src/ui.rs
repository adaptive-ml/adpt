use std::sync::Arc;
use std::time::Duration;

use fancy_duration::AsFancyDuration;
use iocraft::prelude::*;
use uuid::Uuid;

use crate::client::get_job::JobStatusOutput;
use crate::client::list_all_models::{self, ListAllModelsModels};
use crate::client::list_jobs::{self, ListJobsJobsNodes};
use crate::client::list_models::{self, ListModelsUseCaseModelServices};
use crate::client::{AdaptiveClient, get_job};
use crate::client::{
    get_custom_recipes::GetCustomRecipesCustomRecipes,
    get_job::{GetJobJobStages, GetJobJobStagesInfo},
};

#[derive(Default, Props)]
pub struct RecipeListProps {
    pub recipes: Vec<GetCustomRecipesCustomRecipes>,
}

#[component]
pub fn RecipeList(props: &RecipeListProps) -> impl Into<AnyElement<'static>> {
    element! {
        View(flex_direction: FlexDirection::Column) {
            #(props.recipes.iter().map(|recipe| {
                element! {
                        Text(content: recipe.name.clone())
                }
            }))
        }
    }
}

#[derive(Default, Props)]
pub struct JobsListProps {
    pub jobs: Vec<ListJobsJobsNodes>,
}

#[derive(Default, Props)]
pub struct JobStatusIconProps {
    pub status: Option<list_jobs::JobStatus>,
}

#[component]
fn JobStatusIcon(props: &JobStatusIconProps) -> impl Into<AnyElement<'static>> {
    let status = props.status.as_ref().unwrap();
    match status {
        list_jobs::JobStatus::PENDING => element! {
            Text (
                color: Color::Yellow,
                content: "⏳"
            )
        }
        .into_any(),
        list_jobs::JobStatus::RUNNING => element! {
            Text (
                color: Color::Yellow,
                content: "▶️"
            )
        }
        .into_any(),
        list_jobs::JobStatus::COMPLETED => element! {
            Text (
                color: Color::Green,
                content: "✅"
            )
        }
        .into_any(),
        list_jobs::JobStatus::FAILED => element! {
            Text (
                color: Color::Red,
                content: "❌"
            )
        }
        .into_any(),
        list_jobs::JobStatus::CANCELED => element! {
            Text (
                color: Color::Yellow,
                content: "🚫"
            )
        }
        .into_any(),
        list_jobs::JobStatus::Other(_) => element! {
            Text (
                color: Color::Yellow,
                content: "❓"
            )
        }
        .into_any(),
    }
}

trait ModelDisplay {
    fn get_status(&self) -> String;
    fn get_id(&self) -> String;
    fn get_name(&self) -> &str;
    fn get_key(&self) -> &str;
}

impl ModelDisplay for ListModelsUseCaseModelServices {
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

fn render_models_table<T: ModelDisplay + Clone>(models: &[T]) -> impl Into<AnyElement<'static>> {
    element! {
        View(flex_direction: FlexDirection::Column,
             border_style: BorderStyle::Round,
             border_color: Color::Cyan,
        ) {

            View(border_style: BorderStyle::Single, border_edges: Edges::Bottom, border_color: Color::Grey, gap: 2) {
                View(padding_left: 1) {
                    Text(content: "Status", weight: Weight::Bold, decoration: TextDecoration::Underline)
                }

                View(justify_content: JustifyContent::Start, width: 36) {
                    Text(content: "Id", weight: Weight::Bold, decoration: TextDecoration::Underline)
                }

                View(width: 25) {
                    Text(content: "Name", weight: Weight::Bold, decoration: TextDecoration::Underline)
                }

                View(padding_right: 1) {
                    Text(content: "Key", weight: Weight::Bold, decoration: TextDecoration::Underline)
                }
            }
            #({
                if models.is_empty() {
                    vec![element! {
                        View(padding: 2, justify_content: JustifyContent::Center) {
                            Text(content: "No models found", color: Color::Grey)
                        }
                    }]
                } else {
                    models.iter().enumerate().map(|(i, model)| { element! {
                        View(background_color: if i % 2 == 0 { None } else { Some(Color::Grey) }, gap: 2) {
                            View() {
                                Text(content: model.get_status())
                            }

                            View() {
                                Text(content: model.get_id())
                            }

                            View(width: 25) {
                                Text(content: model.get_name())
                            }

                            View(padding_right: 1) {
                                Text(content: model.get_key())
                            }
                        }
                    }
                    }).collect()
                }
            })
        }
    }
}

#[derive(Default, Props)]
pub struct ModelsListProps {
    pub model_services: Vec<ListModelsUseCaseModelServices>,
}

#[component]
pub fn ModelsList(props: &ModelsListProps) -> impl Into<AnyElement<'static>> {
    render_models_table(&props.model_services)
}

#[derive(Default, Props)]
pub struct AllModelsListProps {
    pub models: Vec<ListAllModelsModels>,
}

#[component]
pub fn AllModelsList(props: &AllModelsListProps) -> impl Into<AnyElement<'static>> {
    render_models_table(&props.models)
}

#[component]
pub fn JobsList(props: &JobsListProps) -> impl Into<AnyElement<'static>> {
    element! {
        View(flex_direction: FlexDirection::Column,
             border_style: BorderStyle::Round,
             border_color: Color::Cyan,
        ) {

            View(border_style: BorderStyle::Single, border_edges: Edges::Bottom, border_color: Color::Grey, gap: 2) {
                View(padding_left: 1) {
                    Text(content: "Status", weight: Weight::Bold, decoration: TextDecoration::Underline)
                }

                View(justify_content: JustifyContent::Start, width: 36) {
                    Text(content: "Id", weight: Weight::Bold, decoration: TextDecoration::Underline)
                }

                View() {
                    Text(content: "Duration", weight: Weight::Bold, decoration: TextDecoration::Underline)
                }

                View(padding_right: 1) {
                    Text(content: "User", weight: Weight::Bold, decoration: TextDecoration::Underline)
                }
            }
            #({
                if props.jobs.is_empty() {
                    vec![element! {
                        View(padding: 2, justify_content: JustifyContent::Center) {
                            Text(content: "No jobs found", color: Color::Grey)
                        }
                    }]
                } else {
                    let mut sorted = props.jobs.clone();
                    sorted.sort_by(|job1, job2| job1.created_at.cmp(&job2.created_at).reverse());
                    sorted.into_iter().enumerate().map(|(i, job)| { element! {
                        View(background_color: if i % 2 == 0 { None } else { Some(Color::Grey) }, gap: 2) {
                            View(width: 6, justify_content: JustifyContent::Center, margin_left: 1) {
                                JobStatusIcon(status: job.status.clone())
                            }

                            View() {
                                Text(content: job.id)
                            }

                            View(width: 8) {
                                Text(content: Duration::from_millis(job.duration_ms.unwrap_or_default() as u64).fancy_duration().truncate(2).to_string())
                            }

                            View(padding_right: 1) {
                                Text(content: job.created_by.as_ref().map(|user| format!("{} <{}>", user.name, user.email)).unwrap_or("Unknown".to_string()))
                            }
                        }
                    }
                    }).collect()
                }
            })
        }
    }
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
