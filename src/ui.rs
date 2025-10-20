use std::sync::Arc;

use iocraft::prelude::*;
use uuid::Uuid;

use crate::client::get_job::JobStatusOutput;
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
