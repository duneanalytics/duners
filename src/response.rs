//! Response types for Dune API methods.
//!
//! You will mostly use [`GetResultResponse<T>`] and its [`get_rows`](GetResultResponse::get_rows) method
//! when calling [`refresh`](crate::client::DuneClient::refresh) or [`get_results`](crate::client::DuneClient::get_results).
//! The generic `T` is your row type (a struct with `#[derive(Deserialize)]` matching the query columns).

use crate::parse_utils::{datetime_from_str, optional_datetime_from_str};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_with::DeserializeFromStr;
use std::str::FromStr;

/// Returned from [`DuneClient::execute_query`](crate::client::DuneClient::execute_query). Contains the execution ID to poll or fetch results.
#[derive(Deserialize, Debug)]
pub struct ExecutionResponse {
    pub execution_id: String,
    pub state: ExecutionStatus,
}

/// Represents all possible states of query execution.
/// Most states are self-explanatory.
/// Failure can occur if query takes too long (30 minutes) to execute.
/// Pending state also comes along with a "queue position"
#[derive(DeserializeFromStr, Debug, PartialEq)]
pub enum ExecutionStatus {
    Complete,
    Executing,
    Pending,
    Cancelled,
    Failed,
}

impl FromStr for ExecutionStatus {
    type Err = String;

    fn from_str(input: &str) -> Result<ExecutionStatus, Self::Err> {
        match input {
            "QUERY_STATE_COMPLETED" => Ok(ExecutionStatus::Complete),
            "QUERY_STATE_EXECUTING" => Ok(ExecutionStatus::Executing),
            "QUERY_STATE_PENDING" => Ok(ExecutionStatus::Pending),
            "QUERY_STATE_CANCELLED" => Ok(ExecutionStatus::Cancelled),
            "QUERY_STATE_FAILED" => Ok(ExecutionStatus::Failed),
            other => Err(format!("Parse Error {other}")),
        }
    }
}

impl ExecutionStatus {
    /// utility method for terminal query execution status.
    /// The three terminal states are complete, cancelled and failed.
    pub fn is_terminal(&self) -> bool {
        match self {
            ExecutionStatus::Complete => true,
            ExecutionStatus::Cancelled => true,
            ExecutionStatus::Failed => true,
            ExecutionStatus::Executing => false,
            ExecutionStatus::Pending => false,
        }
    }
}

/// Returned from call to `DuneClient::cancel_execution`
#[derive(Deserialize, Debug)]
pub struct CancellationResponse {
    /// true when cancellation was successful, otherwise false.
    pub success: bool,
}

/// Meta content returned optionally
/// with [GetStatusResponse](GetStatusResponse)
/// and always contained in [ExecutionResult](ExecutionResult).
#[derive(Deserialize, Debug)]
pub struct ResultMetaData {
    pub column_names: Vec<String>,
    #[serde(default)]
    pub column_types: Option<Vec<String>>,
    #[serde(default)]
    pub row_count: Option<u32>,
    pub result_set_bytes: u64,
    #[serde(default)]
    pub total_result_set_bytes: Option<u64>,
    pub total_row_count: u32,
    pub datapoint_count: u32,
    pub pending_time_millis: Option<u32>,
    pub execution_time_millis: u32,
}

/// Nested inside [GetStatusResponse](GetStatusResponse)
/// and [GetResultResponse](GetResultResponse).
/// Contains several UTC timestamps related to the query execution.
#[derive(Deserialize, Debug)]
pub struct ExecutionTimes {
    /// Time when query execution was submitted.
    #[serde(deserialize_with = "datetime_from_str")]
    pub submitted_at: DateTime<Utc>,
    /// Time when execution results will no longer be stored on Dune servers.
    /// None when query execution has not yet completed.
    #[serde(deserialize_with = "optional_datetime_from_str", default)]
    pub expires_at: Option<DateTime<Utc>>,
    /// Time when query execution began.
    /// Differs from `submitted_at` if execution was pending in the queue.
    #[serde(deserialize_with = "optional_datetime_from_str", default)]
    pub execution_started_at: Option<DateTime<Utc>>,
    /// Time that query execution completed.
    #[serde(deserialize_with = "optional_datetime_from_str", default)]
    pub execution_ended_at: Option<DateTime<Utc>>,
    /// Time that query execution was cancelled.
    #[serde(deserialize_with = "optional_datetime_from_str", default)]
    pub cancelled_at: Option<DateTime<Utc>>,
}

/// Returned by successful call to `DuneClient::get_status`.
/// Indicates the current state of execution along with some metadata.
#[derive(Deserialize, Debug)]
pub struct GetStatusResponse {
    pub execution_id: String,
    pub query_id: u32,
    pub state: ExecutionStatus,
    #[serde(flatten)]
    pub times: ExecutionTimes,
    /// If the query state is Pending,
    /// then there will be an associated integer indicating queue position.
    pub queue_position: Option<u32>,
    /// This field will be non-empty once query execution has completed.
    pub result_metadata: Option<ResultMetaData>,
}

/// Contains the query results along with some additional metadata.
/// This struct is nested inside [GetResultResponse](GetResultResponse)
/// as the `result` field.
#[derive(Deserialize, Debug)]
pub struct ExecutionResult<T> {
    pub rows: Vec<T>,
    pub metadata: ResultMetaData,
}

/// Returned by a successful call to `DuneClient::get_results`.
/// Contains similar information to [GetStatusResponse](GetStatusResponse)
/// except that [ResultMetaData](ResultMetaData) is contained within the `result` field.
#[derive(Deserialize, Debug)]
pub struct GetResultResponse<T> {
    pub execution_id: String,
    pub query_id: u32,
    #[serde(default)]
    pub is_execution_finished: Option<bool>,
    pub state: ExecutionStatus,
    // TODO - this `flatten` isn't what I had hoped for.
    //  I want the `times` field to disappear
    //  and all sub-fields to be brought up to this layer.
    #[serde(flatten)]
    pub times: ExecutionTimes,
    pub result: ExecutionResult<T>,
}

impl<T> GetResultResponse<T> {
    /// Convenience method for fetching the "deeply" nested `rows` of the result response.
    pub fn get_rows(self) -> Vec<T> {
        self.result.rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_from_str() {
        assert_eq!(
            ExecutionStatus::from_str("invalid"),
            Err(String::from("Parse Error invalid"))
        );
        assert_eq!(
            ExecutionStatus::from_str("QUERY_STATE_COMPLETED"),
            Ok(ExecutionStatus::Complete)
        );
        assert_eq!(
            ExecutionStatus::from_str("QUERY_STATE_EXECUTING"),
            Ok(ExecutionStatus::Executing)
        );
        assert_eq!(
            ExecutionStatus::from_str("QUERY_STATE_PENDING"),
            Ok(ExecutionStatus::Pending)
        );
        assert_eq!(
            ExecutionStatus::from_str("QUERY_STATE_CANCELLED"),
            Ok(ExecutionStatus::Cancelled)
        );
        assert_eq!(
            ExecutionStatus::from_str("QUERY_STATE_FAILED"),
            Ok(ExecutionStatus::Failed)
        );
    }

    #[test]
    fn terminal_statuses() {
        assert!(ExecutionStatus::Complete.is_terminal());
        assert!(ExecutionStatus::Cancelled.is_terminal());
        assert!(ExecutionStatus::Failed.is_terminal());

        assert!(!ExecutionStatus::Pending.is_terminal());
        assert!(!ExecutionStatus::Executing.is_terminal());
    }
    #[test]
    fn derive_debug() {
        assert_eq!(
            format!(
                "{:?}",
                ExecutionResponse {
                    execution_id: "jerb".to_string(),
                    state: ExecutionStatus::Failed
                }
            ),
            "ExecutionResponse { execution_id: \"jerb\", state: Failed }"
        );
        assert_eq!(
            format!("{:?}", CancellationResponse { success: false }),
            "CancellationResponse { success: false }"
        );
        let query_id = 71;
        let execution_id = "jerb ID";

        assert_eq!(
            format!(
                "{:?}",
                GetStatusResponse {
                    execution_id: execution_id.to_string(),
                    query_id,
                    state: ExecutionStatus::Pending,
                    times: ExecutionTimes {
                        submitted_at: Default::default(),
                        expires_at: Default::default(),
                        execution_started_at: Default::default(),
                        execution_ended_at: Default::default(),
                        cancelled_at: Default::default(),
                    },
                    queue_position: Some(10),
                    result_metadata: Some(ResultMetaData {
                        column_names: vec![],
                        column_types: None,
                        row_count: None,
                        result_set_bytes: 0,
                        total_result_set_bytes: None,
                        total_row_count: 0,
                        datapoint_count: 0,
                        pending_time_millis: None,
                        execution_time_millis: 0,
                    }),
                }
            ),
            "GetStatusResponse { \
                execution_id: \"jerb ID\", \
                query_id: 71, \
                state: Pending, \
                times: ExecutionTimes { \
                    submitted_at: 1970-01-01T00:00:00Z, \
                    expires_at: None, \
                    execution_started_at: None, \
                    execution_ended_at: None, \
                    cancelled_at: None \
                }, \
                queue_position: Some(10), \
                result_metadata: Some(ResultMetaData { \
                        column_names: [], \
                        column_types: None, \
                        row_count: None, \
                        result_set_bytes: 0, \
                        total_result_set_bytes: None, \
                        total_row_count: 0, \
                        datapoint_count: 0, \
                        pending_time_millis: None, \
                        execution_time_millis: 0 \
                }\
             ) }",
        );
        assert_eq!(
            format!(
                "{:?}",
                GetResultResponse {
                    execution_id: execution_id.to_string(),
                    query_id,
                    is_execution_finished: None,
                    state: ExecutionStatus::Complete,
                    times: ExecutionTimes {
                        submitted_at: Default::default(),
                        expires_at: Default::default(),
                        execution_started_at: Default::default(),
                        execution_ended_at: Default::default(),
                        cancelled_at: Default::default(),
                    },
                    result: ExecutionResult::<u8> {
                        rows: vec![],
                        metadata: ResultMetaData {
                            column_names: vec![],
                            column_types: None,
                            row_count: None,
                            result_set_bytes: 0,
                            total_result_set_bytes: None,
                            total_row_count: 0,
                            datapoint_count: 0,
                            pending_time_millis: None,
                            execution_time_millis: 0,
                        }
                    },
                }
            ),
            "GetResultResponse { \
                execution_id: \"jerb ID\", \
                query_id: 71, \
                is_execution_finished: None, \
                state: Complete, \
                times: ExecutionTimes { \
                    submitted_at: 1970-01-01T00:00:00Z, \
                    expires_at: None, \
                    execution_started_at: None, \
                    execution_ended_at: None, \
                    cancelled_at: None \
                }, \
                result: ExecutionResult { \
                    rows: [], \
                    metadata: ResultMetaData { \
                        column_names: [], \
                        column_types: None, \
                        row_count: None, \
                        result_set_bytes: 0, \
                        total_result_set_bytes: None, \
                        total_row_count: 0, \
                        datapoint_count: 0, \
                        pending_time_millis: None, \
                        execution_time_millis: 0 \
                    } \
                } \
            }",
        );
    }
}
