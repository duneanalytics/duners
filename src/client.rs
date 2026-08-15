//! Dune API client implementation.
//!
//! This module provides [`DuneClient`] for calling the [Dune Analytics API](https://dune.com/docs/api/).

use crate::error::{DuneError, DuneRequestError};
use crate::parameters::Parameter;
use crate::response::{
    CancellationResponse, CreateTableRequest, CreateTableResponse, DuneQuery, ExecutionResponse,
    ExecutionStatus, GetResultResponse, GetStatusResponse, InsertTableResponse, QueryBody,
    QueryResponse, SuccessResponse, UploadCsvRequest,
};
use dotenvy::dotenv;
use log::{debug, error, info};
use reqwest::{Error, Response};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::json;
use std::collections::HashMap;
use std::env;
use tokio::time::{sleep, Duration};

/// Base URL for the Dune API (v1).
const BASE_URL: &str = "https://api.dune.com/api/v1";
const DEFAULT_PING_FREQUENCY_SECONDS: u64 = 5;

#[derive(Deserialize)]
struct PaginatedResultResponse<T> {
    #[serde(flatten)]
    response: GetResultResponse<T>,
    #[serde(default)]
    next_offset: Option<u64>,
}

/// Client for the [Dune Analytics API](https://dune.com/docs/api/).
///
/// Create a client with [`DuneClient::new`] (pass the API key directly) or [`DuneClient::from_env`]
/// (reads `DUNE_API_KEY` from the environment, including from a `.env` file if present).
///
/// ## High-level usage
///
/// Use **[`run_query`](DuneClient::run_query)** for a saved query or
/// **[`run_sql`](DuneClient::run_sql)** for raw SQL. Both wait for completion and return all rows.
///
/// ## Low-level usage
///
/// For more control (e.g. polling yourself or cancelling), use:
/// - **[`execute_query`](DuneClient::execute_query)** — Start a query, get an `execution_id`.
/// - **[`get_status`](DuneClient::get_status)** — Check whether the execution is still running.
/// - **[`get_results`](DuneClient::get_results)** — Fetch the result rows (only valid when complete).
/// - **[`cancel_execution`](DuneClient::cancel_execution)** — Cancel a running execution.
pub struct DuneClient {
    /// API key used for request authentication.
    api_key: String,
}

impl DuneClient {
    /// Creates a client with the given API key.
    ///
    /// Get your API key from [Dune → Settings → API](https://dune.com/settings/api).
    pub fn new(api_key: &str) -> DuneClient {
        DuneClient {
            api_key: api_key.to_string(),
        }
    }

    /// Creates a client using the `DUNE_API_KEY` environment variable.
    ///
    /// Loads `.env` from the current directory if present (via the `dotenvy` crate).
    /// Panics if `DUNE_API_KEY` is not set.
    pub fn from_env() -> DuneClient {
        dotenv().ok();
        DuneClient {
            api_key: env::var("DUNE_API_KEY").unwrap(),
        }
    }

    /// Internal POST request handler
    async fn _post(&self, route: &str, params: Option<Vec<Parameter>>) -> Result<Response, Error> {
        let params = params
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.key, p.value))
            .collect::<HashMap<_, _>>();
        let request_url = format!("{BASE_URL}/{route}");
        debug!("POST to {} with parameters {:?}", route, params);
        let client = reqwest::Client::new();
        client
            .post(&request_url)
            .header("x-dune-api-key", &self.api_key)
            .json(&json!({ "query_parameters": params }))
            .send()
            .await
    }

    /// Internal POST request handler with arbitrary JSON body
    async fn _post_json(&self, route: &str, body: serde_json::Value) -> Result<Response, Error> {
        let request_url = format!("{BASE_URL}/{route}");
        debug!("POST to {} with body {:?}", route, body);
        let client = reqwest::Client::new();
        client
            .post(&request_url)
            .header("x-dune-api-key", &self.api_key)
            .json(&body)
            .send()
            .await
    }

    /// Internal PATCH request handler with JSON body
    async fn _patch(&self, route: &str, body: serde_json::Value) -> Result<Response, Error> {
        let request_url = format!("{BASE_URL}/{route}");
        debug!("PATCH to {} with body {:?}", route, body);
        let client = reqwest::Client::new();
        client
            .patch(&request_url)
            .header("x-dune-api-key", &self.api_key)
            .json(&body)
            .send()
            .await
    }

    /// Internal GET request handler for arbitrary routes
    async fn _get_url(&self, route: &str) -> Result<Response, Error> {
        let request_url = format!("{BASE_URL}/{route}");
        debug!("GET from {}", request_url);
        let client = reqwest::Client::new();
        client
            .get(&request_url)
            .header("x-dune-api-key", &self.api_key)
            .send()
            .await
    }

    /// Internal GET request handler for execution endpoints
    async fn _get(&self, job_id: &str, command: &str) -> Result<Response, Error> {
        self._get_url(&format!("execution/{job_id}/{command}"))
            .await
    }

    /// Deserializes Responses into appropriate type.
    /// Some "invalid" requests return response JSON, which are parsed and returned as Errors.
    async fn _parse_response<T: DeserializeOwned>(resp: Response) -> Result<T, DuneRequestError> {
        if resp.status().is_success() {
            resp.json::<T>().await.map_err(DuneRequestError::from)
        } else {
            let err = resp
                .json::<DuneError>()
                .await
                .map_err(DuneRequestError::from)?;
            error!("request error {:?}", err);
            Err(DuneRequestError::from(err))
        }
    }

    /// Internal DELETE request handler
    async fn _delete(&self, route: &str) -> Result<Response, Error> {
        let request_url = format!("{BASE_URL}/{route}");
        debug!("DELETE {}", request_url);
        let client = reqwest::Client::new();
        client
            .delete(&request_url)
            .header("x-dune-api-key", &self.api_key)
            .send()
            .await
    }

    /// Internal POST request handler with raw body and custom content type
    async fn _post_raw(
        &self,
        route: &str,
        content_type: &str,
        body: String,
    ) -> Result<Response, Error> {
        let request_url = format!("{BASE_URL}/{route}");
        debug!("POST raw to {} ({} bytes)", route, body.len());
        let client = reqwest::Client::new();
        client
            .post(&request_url)
            .header("x-dune-api-key", &self.api_key)
            .header("content-type", content_type)
            .body(body)
            .send()
            .await
    }

    /// Parses response body as text (for CSV endpoints).
    async fn _parse_text_response(resp: Response) -> Result<String, DuneRequestError> {
        if resp.status().is_success() {
            resp.text().await.map_err(DuneRequestError::from)
        } else {
            let err = resp
                .json::<DuneError>()
                .await
                .map_err(DuneRequestError::from)?;
            error!("request error {:?}", err);
            Err(DuneRequestError::from(err))
        }
    }

    /// Execute Query (with or without parameters)
    /// cf. [https://dune.com/docs/api/api-reference/execute-queries/execute-query-id/](https://dune.com/docs/api/api-reference/execute-queries/execute-query-id/)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use duners::{DuneClient, DuneRequestError};
    ///
    /// # async fn run() -> Result<(), DuneRequestError> {
    /// let client = DuneClient::from_env();
    /// let exec = client.execute_query(971694, None).await?;
    /// println!("Execution ID: {}", exec.execution_id);
    /// # Ok(()) }
    /// ```
    pub async fn execute_query(
        &self,
        query_id: u32,
        params: Option<Vec<Parameter>>,
    ) -> Result<ExecutionResponse, DuneRequestError> {
        let response = self
            ._post(&format!("query/{query_id}/execute"), params)
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<ExecutionResponse>(response).await
    }

    /// Execute raw SQL directly without a saved query.
    ///
    /// The `performance` parameter controls the execution tier:
    /// `"medium"` (default), `"large"`, or `"community"`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use duners::{DuneClient, DuneRequestError};
    ///
    /// # async fn run() -> Result<(), DuneRequestError> {
    /// let client = DuneClient::from_env();
    /// let exec = client.execute_sql("SELECT 1 AS n", None).await?;
    /// println!("Execution ID: {}", exec.execution_id);
    /// # Ok(()) }
    /// ```
    pub async fn execute_sql(
        &self,
        sql: &str,
        performance: Option<&str>,
    ) -> Result<ExecutionResponse, DuneRequestError> {
        let mut body = json!({ "sql": sql });
        if let Some(perf) = performance {
            body["performance"] = json!(perf);
        }
        let response = self
            ._post_json("sql/execute", body)
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<ExecutionResponse>(response).await
    }

    /// Cancel Query Execution by `job_id`
    /// cf. [https://dune.com/docs/api/api-reference/execute-queries/cancel-execution/](https://dune.com/docs/api/api-reference/execute-queries/cancel-execution/)
    pub async fn cancel_execution(
        &self,
        job_id: &str,
    ) -> Result<CancellationResponse, DuneRequestError> {
        let response = self
            ._post(&format!("execution/{job_id}/cancel"), None)
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<CancellationResponse>(response).await
    }

    /// Get Query Execution Status (by `job_id`)
    /// cf. [https://dune.com/docs/api/api-reference/get-results/execution-status/](https://dune.com/docs/api/api-reference/get-results/execution-status/)
    pub async fn get_status(&self, job_id: &str) -> Result<GetStatusResponse, DuneRequestError> {
        let response = self
            ._get(job_id, "status")
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<GetStatusResponse>(response).await
    }

    /// Get Query Execution Results (by `job_id`)
    /// cf. [https://dune.com/docs/api/api-reference/get-results/execution-results/](https://dune.com/docs/api/api-reference/get-results/execution-results/)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use duners::{DuneClient, DuneRequestError};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize, Debug)]
    /// struct Row { symbol: String, max_price: f64 }
    ///
    /// # async fn run() -> Result<(), DuneRequestError> {
    /// let client = DuneClient::from_env();
    /// let results = client.get_results::<Row>("your-execution-id").await?;
    /// for row in results.get_rows() { println!("{:?}", row); }
    /// # Ok(()) }
    /// ```
    pub async fn get_results<T: DeserializeOwned>(
        &self,
        job_id: &str,
    ) -> Result<GetResultResponse<T>, DuneRequestError> {
        let response = self
            ._get(job_id, "results")
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<GetResultResponse<T>>(response).await
    }

    async fn get_results_page<T: DeserializeOwned>(
        &self,
        job_id: &str,
        offset: Option<u64>,
    ) -> Result<PaginatedResultResponse<T>, DuneRequestError> {
        let route = match offset {
            Some(offset) => format!("execution/{job_id}/results?offset={offset}"),
            None => format!("execution/{job_id}/results"),
        };
        let response = self
            ._get_url(&route)
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<PaginatedResultResponse<T>>(response).await
    }

    /// Get the latest results for a query without triggering a new execution.
    ///
    /// Returns the most recent execution results for the given query ID.
    /// Does not consume credits (no re-execution).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use duners::{DuneClient, DuneRequestError};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize, Debug)]
    /// struct Row { symbol: String, price: f64 }
    ///
    /// # async fn run() -> Result<(), DuneRequestError> {
    /// let client = DuneClient::from_env();
    /// let results = client.get_latest_results::<Row>(971694).await?;
    /// for row in results.get_rows() { println!("{:?}", row); }
    /// # Ok(()) }
    /// ```
    pub async fn get_latest_results<T: DeserializeOwned>(
        &self,
        query_id: u32,
    ) -> Result<GetResultResponse<T>, DuneRequestError> {
        let response = self
            ._get_url(&format!("query/{query_id}/results"))
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<GetResultResponse<T>>(response).await
    }

    /// Get the latest results for a query as CSV text.
    pub async fn get_latest_results_csv(&self, query_id: u32) -> Result<String, DuneRequestError> {
        let response = self
            ._get_url(&format!("query/{query_id}/results/csv"))
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_text_response(response).await
    }

    /// Get execution results as CSV text (by `job_id`).
    pub async fn get_results_csv(&self, job_id: &str) -> Result<String, DuneRequestError> {
        let response = self
            ._get_url(&format!("execution/{job_id}/results/csv"))
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_text_response(response).await
    }

    /// Create a new Dune query.
    ///
    /// `body.name` and `body.query_sql` are required by the API.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use duners::{DuneClient, DuneRequestError, QueryBody};
    ///
    /// # async fn run() -> Result<(), DuneRequestError> {
    /// let client = DuneClient::from_env();
    /// let resp = client.create_query(QueryBody {
    ///     name: Some("My query".into()),
    ///     query_sql: Some("SELECT 1 AS n".into()),
    ///     ..Default::default()
    /// }).await?;
    /// println!("Query ID: {}", resp.query_id);
    /// # Ok(()) }
    /// ```
    pub async fn create_query(&self, body: QueryBody) -> Result<QueryResponse, DuneRequestError> {
        let response = self
            ._post_json("query", serde_json::to_value(&body).unwrap())
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<QueryResponse>(response).await
    }

    /// Read a query's metadata and SQL by ID.
    pub async fn get_query(&self, query_id: u32) -> Result<DuneQuery, DuneRequestError> {
        let response = self
            ._get_url(&format!("query/{query_id}"))
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<DuneQuery>(response).await
    }

    /// Update a query's SQL, name, description, tags, or privacy.
    pub async fn update_query(
        &self,
        query_id: u32,
        body: QueryBody,
    ) -> Result<QueryResponse, DuneRequestError> {
        let response = self
            ._patch(
                &format!("query/{query_id}"),
                serde_json::to_value(&body).unwrap(),
            )
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<QueryResponse>(response).await
    }

    /// Archive a query (prevents running or editing).
    pub async fn archive_query(&self, query_id: u32) -> Result<QueryResponse, DuneRequestError> {
        let response = self
            ._post_json(&format!("query/{query_id}/archive"), json!({}))
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<QueryResponse>(response).await
    }

    /// Unarchive a previously archived query.
    pub async fn unarchive_query(&self, query_id: u32) -> Result<QueryResponse, DuneRequestError> {
        let response = self
            ._post_json(&format!("query/{query_id}/unarchive"), json!({}))
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<QueryResponse>(response).await
    }

    /// Make a query private (owner-only access).
    pub async fn make_query_private(
        &self,
        query_id: u32,
    ) -> Result<QueryResponse, DuneRequestError> {
        let response = self
            ._post_json(&format!("query/{query_id}/private"), json!({}))
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<QueryResponse>(response).await
    }

    /// Make a private query public.
    pub async fn make_query_public(
        &self,
        query_id: u32,
    ) -> Result<QueryResponse, DuneRequestError> {
        let response = self
            ._post_json(&format!("query/{query_id}/unprivate"), json!({}))
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<QueryResponse>(response).await
    }

    /// Create an empty table with an explicit schema.
    pub async fn create_table(
        &self,
        request: CreateTableRequest,
    ) -> Result<CreateTableResponse, DuneRequestError> {
        let response = self
            ._post_json("uploads", serde_json::to_value(&request).unwrap())
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<CreateTableResponse>(response).await
    }

    /// Upload CSV data to create or replace a table.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use duners::{DuneClient, DuneRequestError, UploadCsvRequest};
    ///
    /// # async fn run() -> Result<(), DuneRequestError> {
    /// let client = DuneClient::from_env();
    /// let resp = client.upload_csv(UploadCsvRequest {
    ///     data: "name,age\nAlice,30\nBob,25".into(),
    ///     table_name: "my_table".into(),
    ///     description: None,
    ///     is_private: Some(true),
    /// }).await?;
    /// println!("Table: {}", resp.full_name);
    /// # Ok(()) }
    /// ```
    pub async fn upload_csv(
        &self,
        request: UploadCsvRequest,
    ) -> Result<CreateTableResponse, DuneRequestError> {
        let response = self
            ._post_json("uploads/csv", serde_json::to_value(&request).unwrap())
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<CreateTableResponse>(response).await
    }

    /// Insert rows into an existing table.
    ///
    /// `content_type` should be `"text/csv"` or `"application/x-ndjson"`.
    pub async fn insert_table_rows(
        &self,
        namespace: &str,
        table_name: &str,
        content_type: &str,
        data: String,
    ) -> Result<InsertTableResponse, DuneRequestError> {
        let response = self
            ._post_raw(
                &format!("uploads/{namespace}/{table_name}/insert"),
                content_type,
                data,
            )
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<InsertTableResponse>(response).await
    }

    /// Remove all data from a table (preserves schema).
    pub async fn clear_table(
        &self,
        namespace: &str,
        table_name: &str,
    ) -> Result<SuccessResponse, DuneRequestError> {
        let response = self
            ._post_json(
                &format!("uploads/{namespace}/{table_name}/clear"),
                json!({}),
            )
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<SuccessResponse>(response).await
    }

    /// Permanently delete a table and all its data.
    pub async fn delete_table(
        &self,
        namespace: &str,
        table_name: &str,
    ) -> Result<SuccessResponse, DuneRequestError> {
        let response = self
            ._delete(&format!("uploads/{namespace}/{table_name}"))
            .await
            .map_err(DuneRequestError::from)?;
        DuneClient::_parse_response::<SuccessResponse>(response).await
    }

    /// Execute a saved query, wait for completion, and return all result rows.
    ///
    /// # Arguments
    /// * `query_id` - an integer representing query ID
    ///   (found at the end of a Dune Query URL: [https://dune.com/queries/971694](https://dune.com/queries/971694))
    /// * `parameters` - an optional list of query `Parameter`
    ///   (cf. [https://dune.xyz/queries/3238619](https://dune.xyz/queries/3238619))
    /// * `ping_frequency` - how frequently (in seconds) should the loop check execution status.
    ///   Default is 5 seconds. Too frequently could result in rate limiting
    ///   (i.e. Too Many Requests) especially when executing multiple queries in parallel.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use duners::{DuneClient, DuneRequestError};
    /// use duners::parse_utils::{datetime_from_str, f64_from_str};
    /// use serde::Deserialize;
    /// use chrono::{DateTime, Utc};
    ///
    /// #[derive(Deserialize, Debug)]
    /// struct ResultStruct {
    ///     text_field: String,
    ///     #[serde(deserialize_with = "f64_from_str")]
    ///     number_field: f64,
    ///     #[serde(deserialize_with = "datetime_from_str")]
    ///     date_field: DateTime<Utc>,
    ///     list_field: String,
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), DuneRequestError> {
    ///     let client = DuneClient::from_env();
    ///     let result = client.run_query::<ResultStruct>(1215383, None, None).await?;
    ///     println!("{:?}", result.get_rows());
    ///     Ok(())
    /// }
    /// ```
    pub async fn run_query<T: DeserializeOwned>(
        &self,
        query_id: u32,
        parameters: Option<Vec<Parameter>>,
        ping_frequency: Option<u64>,
    ) -> Result<GetResultResponse<T>, DuneRequestError> {
        let execution = self.execute_query(query_id, parameters).await?;
        info!(
            "Running query {query_id} with execution ID {}",
            execution.execution_id
        );
        self.wait_for_results(&execution.execution_id, ping_frequency)
            .await
    }

    /// Execute raw SQL, wait for completion, and return all result rows.
    ///
    /// The `performance` parameter controls the execution tier. The
    /// `ping_frequency` controls the seconds between status requests and defaults to five.
    pub async fn run_sql<T: DeserializeOwned>(
        &self,
        sql: &str,
        performance: Option<&str>,
        ping_frequency: Option<u64>,
    ) -> Result<GetResultResponse<T>, DuneRequestError> {
        let execution = self.execute_sql(sql, performance).await?;
        info!("Running SQL with execution ID {}", execution.execution_id);
        self.wait_for_results(&execution.execution_id, ping_frequency)
            .await
    }

    /// Compatibility alias for [`run_query`](DuneClient::run_query).
    pub async fn refresh<T: DeserializeOwned>(
        &self,
        query_id: u32,
        parameters: Option<Vec<Parameter>>,
        ping_frequency: Option<u64>,
    ) -> Result<GetResultResponse<T>, DuneRequestError> {
        self.run_query(query_id, parameters, ping_frequency).await
    }

    async fn wait_for_results<T: DeserializeOwned>(
        &self,
        job_id: &str,
        ping_frequency: Option<u64>,
    ) -> Result<GetResultResponse<T>, DuneRequestError> {
        let status = self.poll_until_terminal(job_id, ping_frequency).await?;
        match status.state {
            ExecutionStatus::Complete => self.get_all_results(job_id).await,
            state => Err(terminal_execution_error(job_id, &state)),
        }
    }

    async fn poll_until_terminal(
        &self,
        job_id: &str,
        ping_frequency: Option<u64>,
    ) -> Result<GetStatusResponse, DuneRequestError> {
        let mut status = self.get_status(job_id).await?;
        while !status.state.is_terminal() {
            info!(
                "waiting for query execution {job_id} to complete: {:?}",
                status.state
            );
            sleep(Duration::from_secs(
                ping_frequency.unwrap_or(DEFAULT_PING_FREQUENCY_SECONDS),
            ))
            .await;
            status = self.get_status(job_id).await?
        }
        Ok(status)
    }

    async fn get_all_results<T: DeserializeOwned>(
        &self,
        job_id: &str,
    ) -> Result<GetResultResponse<T>, DuneRequestError> {
        let mut page = self.get_results_page(job_id, None).await?;
        let mut results = page.response;
        while let Some(offset) = page.next_offset {
            if offset == 0 {
                return Err(pagination_error("next offset did not advance"));
            }
            page = self.get_results_page(job_id, Some(offset)).await?;
            merge_result_page(&mut results, &mut page.response)?;
            if page
                .next_offset
                .is_some_and(|next_offset| next_offset <= offset)
            {
                return Err(pagination_error("next offset did not advance"));
            }
        }
        ensure_complete_result(results)
    }
}

fn terminal_execution_error(job_id: &str, state: &ExecutionStatus) -> DuneRequestError {
    DuneRequestError::Dune(format!("execution {job_id} ended in state {state:?}"))
}

fn merge_result_page<T>(
    results: &mut GetResultResponse<T>,
    page: &mut GetResultResponse<T>,
) -> Result<(), DuneRequestError> {
    if results.execution_id != page.execution_id {
        return Err(pagination_error("result execution IDs do not match"));
    }
    if page.state != ExecutionStatus::Complete {
        return Err(pagination_error("result page is not complete"));
    }
    results.result.rows.append(&mut page.result.rows);
    results.result.metadata.row_count = u32::try_from(results.result.rows.len()).ok();
    Ok(())
}

fn pagination_error(message: &str) -> DuneRequestError {
    DuneRequestError::Dune(format!("invalid paginated results: {message}"))
}

fn ensure_complete_result<T>(
    results: GetResultResponse<T>,
) -> Result<GetResultResponse<T>, DuneRequestError> {
    let actual = results.result.rows.len();
    let expected = results.result.metadata.total_row_count as usize;
    if actual == expected {
        Ok(results)
    } else {
        Err(DuneRequestError::Dune(format!(
            "execution {} returned {actual} of {expected} rows",
            results.execution_id
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_utils::{date_parse, datetime_from_str, f64_from_str};
    use crate::response::{ExecutionResult, ExecutionStatus, ExecutionTimes, ResultMetaData};
    use chrono::{DateTime, Utc};
    use serde::Deserialize;
    use tokio::sync::OnceCell;

    const QUERY_ID: u32 = 971694;
    // Long-running query (also used by `long_running_query`): slow enough that a
    // cancellation request reliably lands while it is still executing.
    const SLOW_QUERY_ID: u32 = 1229120;

    fn result_page(rows: Vec<u64>, total_row_count: u32) -> GetResultResponse<u64> {
        GetResultResponse {
            execution_id: "execution-id".to_string(),
            query_id: None,
            is_execution_finished: Some(true),
            state: ExecutionStatus::Complete,
            times: ExecutionTimes {
                submitted_at: Default::default(),
                expires_at: None,
                execution_started_at: None,
                execution_ended_at: None,
                cancelled_at: None,
            },
            result: ExecutionResult {
                rows,
                metadata: ResultMetaData {
                    column_names: vec!["value".to_string()],
                    column_types: None,
                    row_count: Some(1),
                    result_set_bytes: 1,
                    total_result_set_bytes: Some(2),
                    total_row_count,
                    datapoint_count: total_row_count,
                    pending_time_millis: Some(0),
                    execution_time_millis: 1,
                },
            },
        }
    }

    async fn wait_for_completion(dune: &DuneClient, job_id: &str) {
        let mut status = dune.get_status(job_id).await.unwrap();
        while !status.state.is_terminal() {
            sleep(Duration::from_secs(1)).await;
            status = dune.get_status(job_id).await.unwrap();
        }
        assert_eq!(status.state, ExecutionStatus::Complete);
    }

    /// A completed execution created fresh for this test run and shared across tests.
    /// Hardcoded job IDs rot: Dune expires execution results after a retention period.
    async fn fresh_job_id(dune: &DuneClient) -> &'static str {
        static JOB: OnceCell<String> = OnceCell::const_new();
        JOB.get_or_init(|| async {
            let exec = dune
                .execute_sql(
                    "SELECT '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2' AS token, \
                     'WETH' AS symbol, CAST(4200.5 AS double) AS max_price",
                    None,
                )
                .await
                .unwrap();
            wait_for_completion(dune, &exec.execution_id).await;
            exec.execution_id
        })
        .await
    }

    /// Ensures QUERY_ID has a fresh, completed latest execution; without this, the
    /// latest-results endpoints fail once the previous execution's results expire.
    async fn ensure_latest_execution(dune: &DuneClient) {
        static DONE: OnceCell<()> = OnceCell::const_new();
        DONE.get_or_init(|| async {
            let exec = dune.execute_query(QUERY_ID, None).await.unwrap();
            wait_for_completion(dune, &exec.execution_id).await;
        })
        .await;
    }

    #[tokio::test]
    async fn invalid_api_key() {
        let dune = DuneClient::new("Baloney");
        let error = dune.execute_query(QUERY_ID, None).await.unwrap_err();
        assert_eq!(
            error,
            DuneRequestError::Dune(String::from("invalid API Key"))
        )
    }

    #[tokio::test]
    async fn invalid_query_id() {
        let dune = DuneClient::from_env();
        let error = dune.execute_query(u32::MAX, None).await.unwrap_err();
        assert!(matches!(error, DuneRequestError::Dune(_)))
    }

    #[tokio::test]
    async fn invalid_job_id() {
        let dune = DuneClient::from_env();
        let error = dune
            .get_results::<DuneError>("wonky job ID")
            .await
            .unwrap_err();
        assert_eq!(
            error,
            DuneRequestError::Dune(String::from(
                "The requested execution ID (ID: wonky job ID) is invalid."
            ))
        )
    }

    #[tokio::test]
    async fn execute_query() {
        let dune = DuneClient::from_env();
        // Also testing cancellation! Uses SLOW_QUERY_ID: cancelling QUERY_ID here
        // would race with the latest-results tests reading that same query, and a
        // fast query can finish before the cancellation arrives.
        let exec = dune.execute_query(SLOW_QUERY_ID, None).await.unwrap();
        let cancellation = dune.cancel_execution(&exec.execution_id).await.unwrap();
        assert!(cancellation.success);
    }

    #[tokio::test]
    async fn execute_query_with_params() {
        let dune = DuneClient::from_env();
        let all_parameter_types = vec![
            Parameter::date("DateField", date_parse("2022-05-04T00:00:00.0Z").unwrap()),
            Parameter::number("NumberField", "3.1415926535"),
            Parameter::text("TextField", "Plain Text"),
            Parameter::list("ListField", "Option 1"),
        ];
        let exec_result = dune.execute_query(1215383, Some(all_parameter_types)).await;
        assert!(exec_result.is_ok())
    }

    #[tokio::test]
    async fn get_status() {
        let dune = DuneClient::from_env();
        let job_id = fresh_job_id(&dune).await;
        let status = dune.get_status(job_id).await.unwrap();
        assert_eq!(status.state, ExecutionStatus::Complete)
    }

    #[tokio::test]
    async fn get_results() {
        let dune = DuneClient::from_env();

        #[derive(Deserialize, Debug)]
        struct ExpectedResults {
            token: String,
            symbol: String,
            max_price: f64,
        }

        let job_id = fresh_job_id(&dune).await;
        let results = dune.get_results::<ExpectedResults>(job_id).await.unwrap();
        let rows = results.result.rows;
        assert_eq!(1, rows.len());
        assert_eq!(rows[0].symbol, "WETH");
        assert_eq!(rows[0].token, "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        assert_eq!(rows[0].max_price, 4200.5)
    }

    #[tokio::test]
    async fn run_query() {
        let dune = DuneClient::from_env();

        #[derive(Deserialize, Debug, PartialEq)]
        struct ResultStruct {
            text_field: String,
            #[serde(deserialize_with = "f64_from_str")]
            number_field: f64,
            #[serde(deserialize_with = "datetime_from_str")]
            date_field: DateTime<Utc>,
            list_field: String,
        }
        let results = dune
            .run_query::<ResultStruct>(
                3238619,
                Some(vec![Parameter::number("NumberField", "3.141592653589793")]),
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            ResultStruct {
                text_field: "Plain Text".to_string(),
                number_field: std::f64::consts::PI,
                date_field: date_parse("2022-05-04T00:00:00.0Z").unwrap(),
                list_field: "Option 1".to_string(),
            },
            results.get_rows()[0]
        )
    }

    #[tokio::test]
    async fn run_sql() {
        #[derive(Deserialize)]
        struct Row {
            value: u64,
        }

        let results = DuneClient::from_env()
            .run_sql::<Row>("SELECT 1 AS value", None, Some(1))
            .await
            .unwrap();

        assert_eq!(results.query_id, None);
        assert_eq!(results.get_rows()[0].value, 1);
    }

    #[test]
    fn result_pages_are_combined_and_checked_for_completeness() {
        let mut results = result_page(vec![1], 2);
        let mut page = result_page(vec![2], 2);

        merge_result_page(&mut results, &mut page).unwrap();
        let results = ensure_complete_result(results).unwrap();

        assert_eq!(results.result.rows, vec![1, 2]);
        assert_eq!(results.result.metadata.row_count, Some(2));
    }

    #[test]
    fn paginated_envelope_is_deserialized_without_changing_public_response() {
        let page: PaginatedResultResponse<u64> = serde_json::from_value(serde_json::json!({
            "execution_id": "execution-id",
            "query_id": null,
            "state": "QUERY_STATE_COMPLETED",
            "submitted_at": "2026-08-15T00:00:00.000Z",
            "result": {
                "rows": [1],
                "metadata": {
                    "column_names": ["value"],
                    "result_set_bytes": 1,
                    "total_row_count": 2,
                    "datapoint_count": 2,
                    "execution_time_millis": 1
                }
            },
            "next_uri": "https://api.dune.com/api/v1/execution/execution-id/results?offset=1",
            "next_offset": 1
        }))
        .unwrap();

        assert_eq!(page.response.result.rows, vec![1]);
        assert_eq!(page.next_offset, Some(1));
    }

    #[test]
    fn incomplete_results_are_rejected() {
        let error = ensure_complete_result(result_page(vec![1], 2)).unwrap_err();

        assert_eq!(
            error,
            DuneRequestError::Dune("execution execution-id returned 1 of 2 rows".to_string())
        );
    }

    #[test]
    fn unsuccessful_terminal_states_are_errors() {
        assert_eq!(
            terminal_execution_error("execution-id", &ExecutionStatus::Failed),
            DuneRequestError::Dune("execution execution-id ended in state Failed".to_string())
        );
        assert_eq!(
            terminal_execution_error("execution-id", &ExecutionStatus::Cancelled),
            DuneRequestError::Dune("execution execution-id ended in state Cancelled".to_string())
        );
    }

    #[tokio::test]
    async fn table_lifecycle() {
        use crate::response::{ColumnDef, CreateTableRequest};

        let dune = DuneClient::from_env();
        let namespace = env::var("DUNE_NAMESPACE").unwrap_or_else(|_| "bh2smith".to_string());
        let table_name = "duners_test_table";

        // Create table with schema
        let created = dune
            .create_table(CreateTableRequest {
                namespace: namespace.clone(),
                table_name: table_name.to_string(),
                schema: vec![
                    ColumnDef {
                        name: "name".to_string(),
                        column_type: "varchar".to_string(),
                        nullable: None,
                    },
                    ColumnDef {
                        name: "age".to_string(),
                        column_type: "integer".to_string(),
                        nullable: None,
                    },
                ],
                description: None,
                is_private: Some(true),
            })
            .await
            .unwrap();
        assert!(!created.full_name.is_empty());

        // Insert rows via CSV
        let inserted = dune
            .insert_table_rows(
                &namespace,
                table_name,
                "text/csv",
                "name,age\nAlice,30\nBob,25".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(inserted.rows_written, 2);

        // Clear table
        let cleared = dune.clear_table(&namespace, table_name).await.unwrap();
        assert!(cleared.message.is_some());

        // Delete table
        let deleted = dune.delete_table(&namespace, table_name).await.unwrap();
        assert!(deleted.message.is_some());
    }

    #[tokio::test]
    async fn upload_csv_lifecycle() {
        use crate::response::UploadCsvRequest;

        let dune = DuneClient::from_env();
        let namespace = env::var("DUNE_NAMESPACE").unwrap_or_else(|_| "bh2smith".to_string());

        // Upload CSV (creates the table)
        let upload = dune
            .upload_csv(UploadCsvRequest {
                data: "name,age\nAlice,30\nBob,25".to_string(),
                table_name: "duners_csv_test".to_string(),
                description: None,
                is_private: Some(true),
            })
            .await
            .unwrap();
        assert!(!upload.full_name.is_empty());

        // Clean up
        let actual_table = upload.table_name.as_deref().unwrap_or("duners_csv_test");
        dune.delete_table(&namespace, actual_table).await.unwrap();
    }

    #[tokio::test]
    async fn query_crud_lifecycle() {
        use crate::response::QueryBody;

        let dune = DuneClient::from_env();

        // Create
        let created = dune
            .create_query(QueryBody {
                name: Some("duners test query".to_string()),
                query_sql: Some("SELECT 1 AS n".to_string()),
                ..Default::default()
            })
            .await
            .unwrap();
        let qid = created.query_id;
        assert!(qid > 0);

        // Read
        let query = dune.get_query(qid).await.unwrap();
        assert_eq!(query.name, "duners test query");
        assert_eq!(query.query_sql, "SELECT 1 AS n");

        // Update
        let updated = dune
            .update_query(
                qid,
                QueryBody {
                    name: Some("duners test query updated".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.query_id, qid);

        // Make private
        dune.make_query_private(qid).await.unwrap();
        let query = dune.get_query(qid).await.unwrap();
        assert!(query.is_private);

        // Make public
        dune.make_query_public(qid).await.unwrap();
        let query = dune.get_query(qid).await.unwrap();
        assert!(!query.is_private);

        // Archive
        dune.archive_query(qid).await.unwrap();
        let query = dune.get_query(qid).await.unwrap();
        assert!(query.is_archived);

        // Unarchive
        dune.unarchive_query(qid).await.unwrap();
        let query = dune.get_query(qid).await.unwrap();
        assert!(!query.is_archived);

        // Clean up: archive again
        dune.archive_query(qid).await.unwrap();
    }

    #[tokio::test]
    async fn execute_sql() {
        let dune = DuneClient::from_env();
        // No cancellation here: this query finishes so fast that cancelling races
        // ("execution ID not found"); cancellation is covered by `execute_query`.
        let exec = dune.execute_sql("SELECT 1 AS n", None).await.unwrap();
        assert!(!exec.execution_id.is_empty());
    }

    #[tokio::test]
    async fn get_latest_results() {
        let dune = DuneClient::from_env();
        ensure_latest_execution(&dune).await;
        let results = dune
            .get_latest_results::<HashMap<String, serde_json::Value>>(QUERY_ID)
            .await
            .unwrap();
        let rows = results.result.rows;
        assert_eq!(1, rows.len());
        assert_eq!(rows[0]["symbol"], "WETH");
    }

    #[tokio::test]
    async fn get_latest_results_csv() {
        let dune = DuneClient::from_env();
        ensure_latest_execution(&dune).await;
        let csv = dune.get_latest_results_csv(QUERY_ID).await.unwrap();
        assert!(csv.contains("token"));
        assert!(csv.contains("WETH"));
    }

    #[tokio::test]
    async fn get_results_csv() {
        let dune = DuneClient::from_env();
        let job_id = fresh_job_id(&dune).await;
        let csv = dune.get_results_csv(job_id).await.unwrap();
        assert!(csv.contains("token"));
        assert!(csv.contains("WETH"));
    }

    #[tokio::test]
    #[ignore]
    async fn long_running_query() {
        let dune = DuneClient::from_env();
        let results = dune
            .refresh::<HashMap<String, f64>>(SLOW_QUERY_ID, None, None)
            .await
            .unwrap();
        println!("Job ID {:?}", results.execution_id);
        assert_eq!(results.state, ExecutionStatus::Complete);
    }
}
