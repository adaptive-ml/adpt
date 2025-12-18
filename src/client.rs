use std::{fmt::Display, fs::File, io::Read, path::Path, time::SystemTime};

use futures::{StreamExt, stream::BoxStream};
use tokio::sync::mpsc;

use anyhow::{Context, Result, anyhow, bail};
use graphql_client::{GraphQLQuery, Response};
use reqwest::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use url::Url;
use uuid::Uuid;

use crate::rest_types::{
    AbortChunkedUploadRequest, InitChunkedUploadRequest, InitChunkedUploadResponse,
};
use crate::serde_utils;

const MEGABYTE: u64 = 1024 * 1024; // 1MB
pub const MIN_CHUNK_SIZE_BYTES: u64 = 5 * MEGABYTE;
const MAX_CHUNK_SIZE_BYTES: u64 = 100 * MEGABYTE;
const MAX_PARTS_COUNT: u64 = 10000;

const SIZE_500MB: u64 = 500 * MEGABYTE;
const SIZE_10GB: u64 = 10 * 1024 * MEGABYTE;
const SIZE_50GB: u64 = 50 * 1024 * MEGABYTE;

#[derive(Clone, Debug, Default)]
pub struct ChunkedUploadProgress {
    pub bytes_uploaded: u64,
    pub total_bytes: u64,
}

#[derive(Debug)]
pub enum UploadEvent {
    Progress(ChunkedUploadProgress),
    Complete(
        create_dataset_from_multipart::CreateDatasetFromMultipartCreateDatasetFromMultipartUpload,
    ),
}

pub fn calculate_upload_parts(file_size: u64) -> Result<(u64, u64)> {
    if file_size < MIN_CHUNK_SIZE_BYTES {
        bail!(
            "File size ({} bytes) is too small for chunked upload",
            file_size
        );
    }

    let mut chunk_size = if file_size < SIZE_500MB {
        5 * MEGABYTE
    } else if file_size < SIZE_10GB {
        10 * MEGABYTE
    } else if file_size < SIZE_50GB {
        50 * MEGABYTE
    } else {
        100 * MEGABYTE
    };

    let mut total_parts = file_size.div_ceil(chunk_size);

    if total_parts > MAX_PARTS_COUNT {
        chunk_size = file_size.div_ceil(MAX_PARTS_COUNT);

        if chunk_size > MAX_CHUNK_SIZE_BYTES {
            let max_file_size = MAX_CHUNK_SIZE_BYTES * MAX_PARTS_COUNT;
            bail!(
                "File size ({} bytes) exceeds maximum uploadable size ({} bytes = {} parts * {} bytes)",
                file_size,
                max_file_size,
                MAX_PARTS_COUNT,
                MAX_CHUNK_SIZE_BYTES
            );
        }

        total_parts = file_size.div_ceil(chunk_size);
    }

    Ok((total_parts, chunk_size))
}

type IdOrKey = String;
#[allow(clippy::upper_case_acronyms)]
type UUID = Uuid;
type JsObject = Map<String, Value>;
type InputDatetime = String;
#[allow(clippy::upper_case_acronyms)]
type JSON = Value;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(pub SystemTime);

impl<'de> serde::Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let system_time = serde_utils::deserialize_timestamp_millis(deserializer)?;
        Ok(Timestamp(system_time))
    }
}

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

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.gql",
    query_path = "src/graphql/cancel.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CancelJob;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.gql",
    query_path = "src/graphql/models.graphql",
    response_derives = "Debug, Clone"
)]
pub struct ListModels;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.gql",
    query_path = "src/graphql/all_models.graphql",
    response_derives = "Debug, Clone"
)]
pub struct ListAllModels;

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
    query_path = "src/graphql/upload_dataset.graphql",
    response_derives = "Debug, Clone"
)]
pub struct UploadDataset;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.gql",
    query_path = "src/graphql/create_dataset_from_multipart.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CreateDatasetFromMultipart;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.gql",
    query_path = "src/graphql/run.graphql",
    response_derives = "Debug, Clone"
)]
pub struct RunCustomRecipe;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.gql",
    query_path = "src/graphql/usecases.graphql",
    response_derives = "Debug, Clone"
)]
pub struct ListUseCases;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.gql",
    query_path = "src/graphql/pools.graphql",
    response_derives = "Debug, Clone"
)]
pub struct ListComputePools;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.gql",
    query_path = "src/graphql/recipe.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetRecipe;

const INIT_CHUNKED_UPLOAD_ROUTE: &str = "v1/upload/init";
const UPLOAD_PART_ROUTE: &str = "v1/upload/part";
const ABORT_CHUNKED_UPLOAD_ROUTE: &str = "v1/upload/abort";

pub struct AdaptiveClient {
    client: Client,
    graphql_url: Url,
    rest_base_url: Url,
    auth_token: String,
}

impl AdaptiveClient {
    pub fn new(api_base_url: Url, auth_token: String) -> Self {
        let graphql_url = api_base_url
            .join("graphql")
            .expect("Failed to append graphql to base URL");

        Self {
            client: Client::new(),
            graphql_url,
            rest_base_url: api_base_url,
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
            .post(self.graphql_url.clone())
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

    pub async fn upload_dataset<P: AsRef<Path>>(
        &self,
        usecase: &str,
        name: &str,
        dataset: P,
    ) -> Result<upload_dataset::UploadDatasetCreateDataset> {
        let variables = upload_dataset::Variables {
            usecase: IdOrKey::from(usecase),
            file: Upload(0),
            name: Some(name.to_string()),
        };

        let operations = UploadDataset::build_query(variables);
        let operations = serde_json::to_string(&operations)?;

        let file_map = r#"{ "0": ["variables.file"] }"#;

        let dataset_file = reqwest::multipart::Part::file(dataset)
            .await
            .context("Unable to read dataset")?;

        let form = reqwest::multipart::Form::new()
            .text("operations", operations)
            .text("map", file_map)
            .part("0", dataset_file);

        let response = self
            .client
            .post(self.graphql_url.clone())
            .bearer_auth(&self.auth_token)
            .multipart(form)
            .send()
            .await?;

        let response: Response<<UploadDataset as graphql_client::GraphQLQuery>::ResponseData> =
            response.json().await?;

        match response.data {
            Some(data) => Ok(data.create_dataset),
            None => {
                if let Some(errors) = response.errors {
                    bail!("GraphQL errors: {:?}", errors);
                }
                Err(anyhow!("No data returned from GraphQL mutation"))
            }
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
            .post(self.graphql_url.clone())
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
                offset: None,
            }),
        };

        let response_data = self.execute_query(ListJobs, variables).await?;
        Ok(response_data.jobs)
    }

    pub async fn cancel_job(&self, job_id: Uuid) -> Result<cancel_job::CancelJobCancelJob> {
        let variables = cancel_job::Variables { job_id };

        let response_data = self.execute_query(CancelJob, variables).await?;
        Ok(response_data.cancel_job)
    }

    pub async fn list_models(
        &self,
        usecase: String,
    ) -> Result<Vec<list_models::ListModelsUseCaseModelServices>> {
        let variables = list_models::Variables {
            use_case_id: usecase,
        };

        let response_data = self.execute_query(ListModels, variables).await?;
        Ok(response_data
            .use_case
            .map(|use_case| use_case.model_services)
            .unwrap_or(Vec::new()))
    }

    pub async fn list_all_models(&self) -> Result<Vec<list_all_models::ListAllModelsModels>> {
        let variables = list_all_models::Variables {};

        let response_data = self.execute_query(ListAllModels, variables).await?;
        Ok(response_data.models)
    }

    pub async fn list_usecases(&self) -> Result<Vec<list_use_cases::ListUseCasesUseCases>> {
        let variables = list_use_cases::Variables {};

        let response_data = self.execute_query(ListUseCases, variables).await?;
        Ok(response_data.use_cases)
    }

    pub async fn list_pools(
        &self,
    ) -> Result<Vec<list_compute_pools::ListComputePoolsComputePools>> {
        let variables = list_compute_pools::Variables {};

        let response_data = self.execute_query(ListComputePools, variables).await?;
        Ok(response_data.compute_pools)
    }

    pub async fn get_recipe(
        &self,
        usecase: String,
        id_or_key: String,
    ) -> Result<Option<get_recipe::GetRecipeCustomRecipe>> {
        let variables = get_recipe::Variables { usecase, id_or_key };

        let response_data = self.execute_query(GetRecipe, variables).await?;
        Ok(response_data.custom_recipe)
    }

    async fn init_chunked_upload(&self, total_parts: u64) -> Result<String> {
        let url = self
            .rest_base_url
            .join(INIT_CHUNKED_UPLOAD_ROUTE)
            .context("Failed to construct init upload URL")?;

        let request = InitChunkedUploadRequest {
            content_type: "application/jsonl".to_string(),
            metadata: None,
            total_parts_count: total_parts,
        };

        let response = self
            .client
            .post(url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            bail!(
                "Failed to initialize chunked upload: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }

        let init_response: InitChunkedUploadResponse = response.json().await?;
        Ok(init_response.session_id)
    }

    async fn upload_part(
        &self,
        session_id: &str,
        part_number: u64,
        data: Vec<u8>,
        progress_tx: mpsc::Sender<u64>,
    ) -> Result<()> {
        const SUB_CHUNK_SIZE: usize = 64 * 1024;

        let url = self
            .rest_base_url
            .join(UPLOAD_PART_ROUTE)
            .context("Failed to construct upload part URL")?;

        let chunks: Vec<Vec<u8>> = data
            .chunks(SUB_CHUNK_SIZE)
            .map(|chunk| chunk.to_vec())
            .collect();

        let stream = futures::stream::iter(chunks).map(move |chunk| {
            let len = chunk.len() as u64;
            let tx = progress_tx.clone();
            let _ = tx.try_send(len);
            Ok::<_, std::io::Error>(chunk)
        });

        let body = reqwest::Body::wrap_stream(stream);

        let response = self
            .client
            .post(url)
            .bearer_auth(&self.auth_token)
            .query(&[
                ("session_id", session_id),
                ("part_number", &part_number.to_string()),
            ])
            .header("Content-Type", "application/octet-stream")
            .body(body)
            .send()
            .await?;

        if !response.status().is_success() {
            bail!(
                "Failed to upload part {}: {} - {}",
                part_number,
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }

        Ok(())
    }

    async fn abort_chunked_upload(&self, session_id: &str) -> Result<()> {
        let url = self
            .rest_base_url
            .join(ABORT_CHUNKED_UPLOAD_ROUTE)
            .context("Failed to construct abort upload URL")?;

        let request = AbortChunkedUploadRequest {
            session_id: session_id.to_string(),
        };

        let _ = self
            .client
            .delete(url)
            .bearer_auth(&self.auth_token)
            .json(&request)
            .send()
            .await;

        Ok(())
    }

    async fn create_dataset_from_multipart(
        &self,
        usecase: &str,
        name: &str,
        key: &str,
        session_id: &str,
    ) -> Result<
        create_dataset_from_multipart::CreateDatasetFromMultipartCreateDatasetFromMultipartUpload,
    > {
        let variables = create_dataset_from_multipart::Variables {
            input: create_dataset_from_multipart::DatasetCreateFromMultipartUpload {
                use_case: usecase.to_string(),
                name: name.to_string(),
                key: Some(key.to_string()),
                source: None,
                upload_session_id: session_id.to_string(),
            },
        };

        let response_data = self
            .execute_query(CreateDatasetFromMultipart, variables)
            .await?;
        Ok(response_data.create_dataset_from_multipart_upload)
    }

    pub fn chunked_upload_dataset<'a, P: AsRef<Path> + Send + 'a>(
        &'a self,
        usecase: &'a str,
        name: &'a str,
        key: &'a str,
        dataset: P,
    ) -> Result<BoxStream<'a, Result<UploadEvent>>> {
        let file_size = std::fs::metadata(dataset.as_ref())
            .context("Failed to get file metadata")?
            .len();

        let (total_parts, chunk_size) = calculate_upload_parts(file_size)?;

        let stream = async_stream::try_stream! {
            yield UploadEvent::Progress(ChunkedUploadProgress {
                bytes_uploaded: 0,
                total_bytes: file_size,
            });

            let session_id = self.init_chunked_upload(total_parts).await?;

            let mut file =
                File::open(dataset.as_ref()).context("Failed to open dataset file")?;
            let mut buffer = vec![0u8; chunk_size as usize];
            let mut bytes_uploaded = 0u64;

            let (progress_tx, mut progress_rx) = mpsc::channel::<u64>(64);

            for part_number in 1..=total_parts {
                let bytes_read = file.read(&mut buffer).context("Failed to read chunk")?;
                let chunk_data = buffer[..bytes_read].to_vec();

                let upload_fut = self.upload_part(&session_id, part_number, chunk_data, progress_tx.clone());
                tokio::pin!(upload_fut);

                let upload_result: Result<()> = loop {
                    tokio::select! {
                        biased;
                        result = &mut upload_fut => {
                            break result;
                        }
                        Some(bytes) = progress_rx.recv() => {
                            bytes_uploaded += bytes;
                            yield UploadEvent::Progress(ChunkedUploadProgress {
                                bytes_uploaded,
                                total_bytes: file_size,
                            });
                        }
                    }
                };

                if let Err(e) = upload_result {
                    let _ = self.abort_chunked_upload(&session_id).await;
                    Err(e)?;
                }
            }

            let create_result = self
                .create_dataset_from_multipart(usecase, name, key, &session_id)
                .await;

            match create_result {
                Ok(response) => {
                    yield UploadEvent::Complete(response);
                }
                Err(e) => {
                    let _ = self.abort_chunked_upload(&session_id).await;
                    Err(anyhow!("Failed to create dataset: {}", e))?;
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
