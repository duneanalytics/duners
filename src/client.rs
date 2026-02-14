use crate::error::{DuneError, DuneRequestError};
use crate::parameters::Parameter;
use crate::response::{
    CancellationResponse, ExecutionResponse, ExecutionStatus, GetResultResponse, GetStatusResponse,
};
use dotenv::dotenv;
use log::{debug, error, info, warn};
use reqwest::{Error, Response};
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use tokio::time::{sleep, Duration};

const BASE_URL: &str = "https://api.dune.com/api/v1";

/// DuneClient provides an interface for interacting with Dune Analytics API.
/// Official Documentation here: [https://dune.com/docs/api/](https://dune.com/docs/api/).
///
/// Elementary Routes (i.e. those provided by Dune)
/// - POST
///     - execute_query
///     - cancel_execution
/// - GET
///     - get_status
///     - get_results
///
/// Furthermore, this interface also implements a convenience method `refresh` which acts as follows:
/// 1. Execute query
/// 2. While execution status is not in a terminal state, sleep and check again
/// 3. Get and return execution results.
pub struct DuneClient {
    /// An essential value for request authentication.
    api_key: String,
}

impl DuneClient {
    /// Constructor
    pub fn new(api_key: &str) -> DuneClient {
        DuneClient {
            api_key: api_key.to_string(),
        }
    }
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
        debug!("POST to {} with parameters {:?}", route, &params);
        let client = reqwest::Client::new();
        client
            .post(&request_url)
            .header("x-dune-api-key", &self.api_key)
            .json(&json!({ "query_parameters": params }))
            .send()
            .await
    }

    /// Internal GET request handler
    async fn _get(&self, job_id: &str, command: &str) -> Result<Response, Error> {
        let request_url = format!("{BASE_URL}/execution/{job_id}/{command}");
        debug!("GET from {}", &request_url);
        let client = reqwest::Client::new();
        client
            .get(&request_url)
            .header("x-dune-api-key", &self.api_key)
            .send()
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
            error!("request error {:?}", &err);
            Err(DuneRequestError::from(err))
        }
    }

    /// Execute Query (with or without parameters)
    /// cf. [https://dune.com/docs/api/api-reference/execute-queries/execute-query-id/](https://dune.com/docs/api/api-reference/execute-queries/execute-query-id/)
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

    /// Convenience method for users to
    /// 1. execute,
    /// 2. wait for execution to complete,
    /// 3. fetch and return query results.
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
    /// ```
    /// use duners::{
    ///     client::DuneClient,
    ///     parse_utils::{datetime_from_str, f64_from_str},
    ///     error::DuneRequestError
    /// };
    /// use serde::Deserialize;
    /// use chrono::{DateTime, Utc};
    ///
    /// // User must declare the expected query return types and fields.
    /// #[derive(Deserialize, Debug, PartialEq)]
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
    ///     let dune = DuneClient::from_env();
    ///     let results = dune.refresh::<ResultStruct>(1215383, None, None).await?;
    ///     println!("{:?}", results.get_rows());
    ///     Ok(())
    /// }
    /// ```
    pub async fn refresh<T: DeserializeOwned>(
        &self,
        query_id: u32,
        parameters: Option<Vec<Parameter>>,
        ping_frequency: Option<u64>,
    ) -> Result<GetResultResponse<T>, DuneRequestError> {
        let job_id = self.execute_query(query_id, parameters).await?.execution_id;
        info!("Refreshing {} Execution ID {}", query_id, job_id);
        let mut status = self.get_status(&job_id).await?;
        while !status.state.is_terminal() {
            info!(
                "waiting for query execution {job_id} to complete: {:?}",
                status.state
            );
            sleep(Duration::from_secs(ping_frequency.unwrap_or(5))).await;
            status = self.get_status(&job_id).await?
        }
        let full_response = self.get_results::<T>(&job_id).await;
        if status.state == ExecutionStatus::Failed {
            warn!(
                "{:?} Perhaps your query took too long to run!",
                status.state
            );
        }
        full_response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_utils::{date_parse, datetime_from_str, f64_from_str};
    use crate::response::ExecutionStatus;
    use chrono::{DateTime, Utc};
    use serde::Deserialize;

    const QUERY_ID: u32 = 971694;
    const JOB_ID: &str = "01KHDCT5QFS1QPE9T2QEWPEAGG";

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
        assert_eq!(
            error,
            DuneRequestError::Dune(String::from("An internal error occurred"))
        )
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
        let exec = dune.execute_query(QUERY_ID, None).await.unwrap();
        // Also testing cancellation!
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
        let status = dune.get_status(JOB_ID).await.unwrap();
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

        let results = dune.get_results::<ExpectedResults>(JOB_ID).await.unwrap();
        // Query is for the max ETH price (should only have 1 result)
        let rows = results.result.rows;
        assert_eq!(1, rows.len());
        assert_eq!(rows[0].symbol, "WETH");
        assert_eq!(rows[0].token, "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        assert!(rows[0].max_price > 4148.0)
    }

    #[tokio::test]
    async fn refresh() {
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
            .refresh::<ResultStruct>(
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
    #[ignore]
    async fn long_running_query() {
        let dune = DuneClient::from_env();
        let results = dune
            .refresh::<HashMap<String, f64>>(1229120, None, None)
            .await
            .unwrap();
        println!("Job ID {:?}", results.execution_id);
        assert_eq!(results.state, ExecutionStatus::Complete);
    }
}
