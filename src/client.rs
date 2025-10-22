use std::{fmt::Display, path::Path};

use anyhow::{Context, Result, anyhow, bail};
use graphql_client::{GraphQLQuery, Response};
use reqwest::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use url::Url;
use uuid::Uuid;

type IdOrKey = String;
#[allow(clippy::upper_case_acronyms)]
type UUID = Uuid;
//FIXME make instant
type Timestamp = u64;
type JsObject = Map<String, Value>;
type InputDatetime = String;

const PAGE_SIZE: usize = 20;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Upload(usize);

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.gql",
    query_path = "src/graphql/list.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetCustomRecipes;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.gql",
    query_path = "src/graphql/job.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetJob;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.gql",
    query_path = "src/graphql/jobs.graphql",
    response_derives = "Debug, Clone"
)]
pub struct ListJobs;

impl Display for get_job::JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            get_job::JobStatus::PENDING => write!(f, "Pending"),
            get_job::JobStatus::RUNNING => write!(f, "Running"),
            get_job::JobStatus::COMPLETED => write!(f, "Completed"),
            get_job::JobStatus::FAILED => write!(f, "Failed"),
            get_job::JobStatus::CANCELED => write!(f, "Canceled"),
            get_job::JobStatus::Other(_) => write!(f, "Unknown"),
        }
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.gql",
    query_path = "src/graphql/publish.graphql",
    response_derives = "Debug, Clone"
)]
pub struct PublishCustomRecipe;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.gql",
    query_path = "src/graphql/run.graphql",
    response_derives = "Debug, Clone"
)]
pub struct RunCustomRecipe;

pub struct AdaptiveClient {
    client: Client,
    base_url: Url,
    auth_token: String,
}

impl AdaptiveClient {
    pub fn new(base_url: Url, auth_token: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
            auth_token,
        }
    }

    async fn execute_query<T>(&self, _query: T, variables: T::Variables) -> Result<T::ResponseData>
    where
        T: GraphQLQuery,
        T::Variables: serde::Serialize,
        T::ResponseData: DeserializeOwned,
    {
        let request_body = T::build_query(variables);

        let response = self
            .client
            .post(self.base_url.clone())
            .bearer_auth(&self.auth_token)
            .json(&request_body)
            .send()
            .await?;

        let response_body: Response<T::ResponseData> = response.json().await?;

        match response_body.data {
            Some(data) => Ok(data),
            None => {
                if let Some(errors) = response_body.errors {
                    bail!("GraphQL errors: {:?}", errors);
                }
                Err(anyhow!("No data returned from GraphQL query"))
            }
        }
    }

    pub async fn list_recipes(
        &self,
        usecase: &str,
    ) -> Result<Vec<get_custom_recipes::GetCustomRecipesCustomRecipes>> {
        let variables = get_custom_recipes::Variables {
            usecase: IdOrKey::from(usecase),
        };

        let response_data = self.execute_query(GetCustomRecipes, variables).await?;
        Ok(response_data.custom_recipes)
    }

    pub async fn get_job(&self, job_id: Uuid) -> Result<get_job::GetJobJob> {
        let variables = get_job::Variables { id: job_id };

        let response_data = self.execute_query(GetJob, variables).await?;

        match response_data.job {
            Some(job) => Ok(job),
            None => Err(anyhow!("Job with ID '{}' not found", job_id)),
        }
    }

    pub async fn publish_recipe<P: AsRef<Path>>(
        &self,
        usecase: &str,
        name: &str,
        key: &str,
        recipe: P,
    ) -> Result<publish_custom_recipe::PublishCustomRecipeCreateCustomRecipe> {
        let variables = publish_custom_recipe::Variables {
            usecase: IdOrKey::from(usecase),
            file: Upload(0),
            name: Some(name.to_string()),
            key: Some(key.to_string()),
        };

        let operations = PublishCustomRecipe::build_query(variables);
        let operations = serde_json::to_string(&operations)?;

        let file_map = r#"{ "0": ["variables.file"] }"#;

        let recipe_file = reqwest::multipart::Part::file(recipe)
            .await
            .context("Unable to read recipe")?;

        let form = reqwest::multipart::Form::new()
            .text("operations", operations)
            .text("map", file_map)
            .part("0", recipe_file);

        let response = self
            .client
            .post(self.base_url.clone())
            .bearer_auth(&self.auth_token)
            .multipart(form)
            .send()
            .await?;
        let response: Response<
            <PublishCustomRecipe as graphql_client::GraphQLQuery>::ResponseData,
        > = response.json().await?;

        match response.data {
            Some(data) => Ok(data.create_custom_recipe),
            None => {
                if let Some(errors) = response.errors {
                    bail!("GraphQL errors: {:?}", errors);
                }
                Err(anyhow!("No data returned from GraphQL mutation"))
            }
        }
    }

    pub async fn run_recipe(
        &self,
        usecase: &str,
        recipe_id: &str,
        parameters: Map<String, Value>,
        name: Option<String>,
        compute_pool: Option<String>,
        num_gpus: u32,
    ) -> Result<run_custom_recipe::RunCustomRecipeCreateJob> {
        let variables = run_custom_recipe::Variables {
            input: run_custom_recipe::JobInput {
                recipe: recipe_id.to_string(),
                use_case: usecase.to_string(),
                args: parameters,
                name,
                compute_pool,
                num_gpus: num_gpus as i64,
            },
        };

        let response_data = self.execute_query(RunCustomRecipe, variables).await?;
        Ok(response_data.create_job)
    }

    pub async fn list_jobs(
        &self,
        usecase: Option<String>,
    ) -> Result<Vec<list_jobs::ListJobsJobsNodes>> {
        let mut jobs = Vec::new();
        let mut page = self.list_jobs_page(usecase.clone(), None).await?;
        jobs.extend(page.nodes.iter().cloned());
        while page.page_info.has_next_page {
            page = self
                .list_jobs_page(usecase.clone(), page.page_info.end_cursor)
                .await?;
            jobs.extend(page.nodes.iter().cloned());
        }
        Ok(jobs)
    }

    async fn list_jobs_page(
        &self,
        usecase: Option<String>,
        after: Option<String>,
    ) -> Result<list_jobs::ListJobsJobs> {
        let variables = list_jobs::Variables {
            filter: Some(list_jobs::ListJobsFilterInput {
                use_case: usecase,
                kind: Some(vec![list_jobs::JobKind::CUSTOM]),
                status: Some(vec![
                    list_jobs::JobStatus::RUNNING,
                    list_jobs::JobStatus::PENDING,
                ]),
                timerange: None,
                custom_recipes: None,
                artifacts: None,
            }),
            cursor: Some(list_jobs::CursorPageInput {
                first: Some(PAGE_SIZE as i64),
                after,
                before: None,
                last: None,
            }),
        };

        let response_data = self.execute_query(ListJobs, variables).await?;
        Ok(response_data.jobs)
    }
}
