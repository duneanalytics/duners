//! Response types for Dune API methods.
//!
//! You will mostly use [`GetResultResponse<T>`] and its [`get_rows`](GetResultResponse::get_rows) method
//! when calling [`run_query`](crate::client::DuneClient::run_query),
//! [`run_sql`](crate::client::DuneClient::run_sql), or
//! [`get_results`](crate::client::DuneClient::get_results).
//! The generic `T` is your row type (a struct with `#[derive(Deserialize)]` matching the query columns).

use crate::parse_utils::{datetime_from_str, optional_datetime_from_str};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::DeserializeFromStr;
use std::str::FromStr;

#[derive(Deserialize)]
pub(crate) struct PaginatedResultResponse<T> {
    #[serde(flatten)]
    pub(crate) response: GetResultResponse<T>,
    #[serde(default)]
    pub(crate) next_offset: Option<u64>,
}

/// Returned from [`DuneClient::execute_query`](crate::client::DuneClient::execute_query). Contains the execution ID to poll or fetch results.
#[derive(Deserialize, Debug)]
pub struct ExecutionResponse {
    /// Use this ID with [`get_status`](crate::client::DuneClient::get_status) and [`get_results`](crate::client::DuneClient::get_results).
    pub execution_id: String,
    /// Current state of the execution (e.g. [`ExecutionStatus::Pending`]).
    pub state: ExecutionStatus,
}

/// Represents all possible states of query execution.
/// Most states are self-explanatory.
/// Failure can occur if query takes too long (30 minutes) to execute.
/// Pending state also comes along with a "queue position"
#[derive(DeserializeFromStr, Debug, PartialEq)]
pub enum ExecutionStatus {
    /// Query finished successfully; results are available.
    Complete,
    /// Query is currently running.
    Executing,
    /// Query is queued; check `queue_position` on [`GetStatusResponse`].
    Pending,
    /// Execution was cancelled (e.g. via [`cancel_execution`](crate::client::DuneClient::cancel_execution)).
    Cancelled,
    /// Execution failed (e.g. timeout after 30 minutes).
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
    /// Returns `true` when execution will not change state again (complete, cancelled, or failed).
    ///
    /// # Example
    ///
    /// ```rust
    /// use duners::ExecutionStatus;
    ///
    /// assert!(ExecutionStatus::Complete.is_terminal());
    /// assert!(!ExecutionStatus::Pending.is_terminal());
    /// ```
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
    /// Names of columns in the result set.
    pub column_names: Vec<String>,
    /// Optional Dune type names for each column.
    #[serde(default)]
    pub column_types: Option<Vec<String>>,
    /// Number of rows in this result set (when present).
    #[serde(default)]
    pub row_count: Option<u32>,
    /// Size in bytes of the result set.
    pub result_set_bytes: u64,
    /// Total size when result is paged.
    #[serde(default)]
    pub total_result_set_bytes: Option<u64>,
    /// Total number of rows across all pages.
    pub total_row_count: u32,
    /// Number of datapoints (Dune-specific).
    pub datapoint_count: u32,
    /// Time spent in queue before execution started (milliseconds).
    pub pending_time_millis: Option<u32>,
    /// Time spent executing the query (milliseconds).
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
    /// Same execution ID used in the status request.
    pub execution_id: String,
    /// The Dune query ID that was executed.
    pub query_id: u32,
    /// Current execution state; use [`ExecutionStatus::is_terminal`] to check if done.
    pub state: ExecutionStatus,
    /// Timestamps for submitted_at, expires_at, execution_started_at, etc.
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
    /// Deserialized result rows; `T` is your row type (e.g. a struct with `#[derive(Deserialize)]`).
    pub rows: Vec<T>,
    /// Column names, row counts, and timing info.
    pub metadata: ResultMetaData,
}

/// Returned by a successful call to `DuneClient::get_results`.
/// Contains similar information to [GetStatusResponse](GetStatusResponse)
/// except that [ResultMetaData](ResultMetaData) is contained within the `result` field.
#[derive(Deserialize, Debug)]
pub struct GetResultResponse<T> {
    /// Execution ID for this result.
    pub execution_id: String,
    /// The Dune query ID that was executed.
    /// `None` for raw-SQL executions (see `DuneClient::execute_sql`), which have no saved query.
    #[serde(default)]
    pub query_id: Option<u32>,
    /// Optional flag indicating whether execution is finished.
    #[serde(default)]
    pub is_execution_finished: Option<bool>,
    /// Final state (typically [`ExecutionStatus::Complete`] when results are available).
    pub state: ExecutionStatus,
    // TODO - this `flatten` isn't what I had hoped for.
    //  I want the `times` field to disappear
    //  and all sub-fields to be brought up to this layer.
    /// Timestamps for submitted_at, expires_at, execution_started_at, etc.
    #[serde(flatten)]
    pub times: ExecutionTimes,
    /// The result set (rows and metadata).
    pub result: ExecutionResult<T>,
}

impl<T> GetResultResponse<T> {
    /// Convenience method for fetching the "deeply" nested `rows` of the result response.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use duners::{DuneClient, DuneRequestError, GetResultResponse};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct Row { symbol: String, max_price: f64 }
    ///
    /// # async fn run() -> Result<(), DuneRequestError> {
    /// let client = DuneClient::from_env();
    /// let response: GetResultResponse<Row> = client.run_query(971694, None, None).await?;
    /// let rows = response.get_rows();
    /// # Ok(()) }
    /// ```
    pub fn get_rows(self) -> Vec<T> {
        self.result.rows
    }
}

/// Response from query create, update, archive, unarchive, private, and unprivate endpoints.
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
///     query_sql: Some("SELECT 1".into()),
///     ..Default::default()
/// }).await?;
/// println!("Created query {}", resp.query_id);
/// # Ok(()) }
/// ```
#[derive(Deserialize, Debug)]
pub struct QueryResponse {
    /// The Dune query ID.
    pub query_id: u32,
}

/// Full query object returned by `GET /v1/query/{queryId}`.
///
/// # Example
///
/// ```no_run
/// use duners::{DuneClient, DuneRequestError};
///
/// # async fn run() -> Result<(), DuneRequestError> {
/// let client = DuneClient::from_env();
/// let query = client.get_query(971694).await?;
/// println!("{}: {}", query.name, query.query_sql);
/// # Ok(()) }
/// ```
#[derive(Deserialize, Debug)]
pub struct DuneQuery {
    /// The Dune query ID.
    pub query_id: u32,
    /// Human-readable query name.
    pub name: String,
    /// Optional description of the query.
    #[serde(default)]
    pub description: Option<String>,
    /// The SQL text of the query.
    pub query_sql: String,
    /// Whether the query is private (owner-only).
    pub is_private: bool,
    /// Whether the query is archived.
    pub is_archived: bool,
    /// SQL engine used (e.g. `"medium"`).
    #[serde(default)]
    pub query_engine: Option<String>,
    /// Query version number.
    #[serde(default)]
    pub version: Option<u32>,
    /// User-defined tags.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Query parameters defined in the Dune editor.
    #[serde(default)]
    pub parameters: Option<Vec<serde_json::Value>>,
}

/// Request body for creating or updating a query.
///
/// For [`create_query`](crate::client::DuneClient::create_query), `name` and `query_sql` are required by the API.
/// For [`update_query`](crate::client::DuneClient::update_query), all fields are optional.
///
/// # Example
///
/// ```rust
/// use duners::QueryBody;
///
/// let body = QueryBody {
///     name: Some("My query".into()),
///     query_sql: Some("SELECT 1 AS n".into()),
///     ..Default::default()
/// };
/// ```
#[derive(Serialize, Debug, Default)]
pub struct QueryBody {
    /// Query name (required for create).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// SQL text (required for create).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_sql: Option<String>,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the query should be private.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_private: Option<bool>,
    /// User-defined tags.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// Column definition for creating a table.
///
/// # Example
///
/// ```rust
/// use duners::ColumnDef;
///
/// let col = ColumnDef {
///     name: "price".into(),
///     column_type: "double".into(),
///     nullable: Some(true),
/// };
/// ```
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ColumnDef {
    /// Column name.
    pub name: String,
    /// Dune column type (e.g. `"varchar"`, `"integer"`, `"double"`, `"boolean"`).
    #[serde(rename = "type")]
    pub column_type: String,
    /// Whether the column accepts NULL values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nullable: Option<bool>,
}

/// Request body for `POST /v1/uploads` (create empty table with schema).
///
/// # Example
///
/// ```no_run
/// use duners::{DuneClient, DuneRequestError, CreateTableRequest, ColumnDef};
///
/// # async fn run() -> Result<(), DuneRequestError> {
/// let client = DuneClient::from_env();
/// let resp = client.create_table(CreateTableRequest {
///     namespace: "my_team".into(),
///     table_name: "prices".into(),
///     schema: vec![
///         ColumnDef { name: "symbol".into(), column_type: "varchar".into(), nullable: None },
///         ColumnDef { name: "price".into(), column_type: "double".into(), nullable: None },
///     ],
///     description: None,
///     is_private: Some(true),
/// }).await?;
/// println!("Created: {}", resp.full_name);
/// # Ok(()) }
/// ```
#[derive(Serialize, Debug)]
pub struct CreateTableRequest {
    /// Dune namespace (usually your username or team name).
    pub namespace: String,
    /// Table name.
    pub table_name: String,
    /// Column definitions.
    pub schema: Vec<ColumnDef>,
    /// Optional table description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the table should be private.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_private: Option<bool>,
}

/// Response from `POST /v1/uploads` and `POST /v1/uploads/csv`.
#[derive(Deserialize, Debug)]
pub struct CreateTableResponse {
    /// Whether the operation succeeded (present for CSV uploads).
    #[serde(default)]
    pub success: Option<bool>,
    /// Dune namespace.
    #[serde(default)]
    pub namespace: Option<String>,
    /// Table name (may include a `dataset_` prefix for CSV uploads).
    #[serde(default)]
    pub table_name: Option<String>,
    /// Fully qualified table name (e.g. `dune.my_team.my_table`).
    #[serde(default)]
    pub full_name: String,
    /// Example SQL query to read from this table.
    #[serde(default)]
    pub example_query: Option<String>,
    /// Whether the table already existed before this call.
    #[serde(default)]
    pub already_existed: Option<bool>,
    /// Additional message from the API.
    #[serde(default)]
    pub message: Option<String>,
}

/// Request body for `POST /v1/uploads/csv`.
#[derive(Serialize, Debug)]
pub struct UploadCsvRequest {
    /// CSV content as a string (including header row).
    pub data: String,
    /// Table name to create or replace.
    pub table_name: String,
    /// Optional table description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the table should be private.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_private: Option<bool>,
}

/// Response from `POST /v1/uploads/{namespace}/{table_name}/insert`.
#[derive(Deserialize, Debug)]
pub struct InsertTableResponse {
    /// Number of rows written.
    pub rows_written: u64,
    /// Total bytes written.
    pub bytes_written: u64,
    /// Table name.
    #[serde(default)]
    pub name: Option<String>,
}

/// Generic success response with a message field (used by clear and delete table).
#[derive(Deserialize, Debug)]
pub struct SuccessResponse {
    /// Human-readable message from the API.
    #[serde(default)]
    pub message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_submission_input_omits_unset_optionals_and_keeps_abi_shape() {
        let input = ContractSubmissionInput {
            blockchain_name: "ethereum".into(),
            address: "0x1".into(),
            project_name: "p".into(),
            contract_name: "c".into(),
            abi: serde_json::json!([{"type": "event"}]),
            is_proxy: true,
            idempotency_key: Some("k/0".into()),
            ..Default::default()
        };
        let value = serde_json::to_value(SubmitContractsRequest {
            submissions: vec![input],
        })
        .unwrap();
        let item = &value["submissions"][0];
        assert_eq!(item["abi"], serde_json::json!([{"type": "event"}]));
        assert_eq!(item["is_proxy"], true);
        assert_eq!(item["has_multiple_instances"], false);
        assert_eq!(item["idempotency_key"], "k/0");
        assert!(item.get("submission_type").is_none());
        assert!(item.get("resubmission_reason").is_none());

        let delete = ContractSubmissionInput {
            abi: serde_json::Value::String("[]".into()),
            submission_type: Some(ContractSubmissionType::Delete),
            resubmission_reason: Some("redeployed".into()),
            ..Default::default()
        };
        let value = serde_json::to_value(&delete).unwrap();
        assert_eq!(value["abi"], "[]");
        assert_eq!(value["submission_type"], "delete");
    }

    #[test]
    fn parses_submit_and_list_responses() {
        let submit: SubmitContractsResponse = serde_json::from_str(
            r#"{"results":[
                {"index":0,"submission_id":"sub_1","status":"pending"},
                {"index":1,"submission_id":"sub_0","status":"pending","replayed":true},
                {"index":2,"error":"abi must be valid JSON"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(submit.results.len(), 3);
        assert_eq!(submit.results[0].submission_id.as_deref(), Some("sub_1"));
        assert!(!submit.results[0].replayed);
        assert!(submit.results[1].replayed);
        assert_eq!(
            submit.results[2].error.as_deref(),
            Some("abi must be valid JSON")
        );
        assert!(submit.results[2].submission_id.is_none());

        let list: ListContractSubmissionsResponse = serde_json::from_str(
            r#"{"submissions":[{
                "id":"sub_1","blockchain_name":"ethereum","address":"0x1",
                "project_name":"p","contract_name":"c","status":"needs_manual_review",
                "submission_type":"upgrade","comment":"protected namespace",
                "created_at":"2026-09-10T11:04:18.724658Z","updated_at":"2026-09-10T11:05:18Z",
                "idempotency_key":"k/1"
            }],"total":7,"next_cursor":"abc"}"#,
        )
        .unwrap();
        assert_eq!(list.total, 7);
        assert_eq!(list.next_cursor.as_deref(), Some("abc"));
        let s = &list.submissions[0];
        assert_eq!(s.status, ContractSubmissionStatus::NeedsManualReview);
        assert_eq!(s.submission_type, ContractSubmissionType::Upgrade);
        assert_eq!(s.comment.as_deref(), Some("protected namespace"));
        assert_eq!(s.created_at.timestamp(), 1789038258);
    }

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
                    query_id: Some(query_id),
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
                query_id: Some(71), \
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

/// The kind of change a contract decoding submission describes.
///
/// Upgrades, renames, deletions and "other" requests are always routed to manual review.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractSubmissionType {
    /// Decode a contract that is not yet decoded under this project and name.
    New,
    /// Replace the ABI of an already decoded contract.
    Upgrade,
    /// Move a decoded contract to a new project and/or contract name.
    Rename,
    /// Remove a decoded contract.
    Delete,
    /// A free-form request explained in `resubmission_reason`.
    Other,
}

/// Lifecycle status of a contract decoding submission.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractSubmissionStatus {
    /// Queued; the decoding pipeline picks it up within about a minute.
    Pending,
    /// Validated and about to be decoded.
    Approved,
    /// Not accepted; see `comment`.
    Rejected,
    /// Decoded tables are available.
    Processed,
    /// Being handled manually.
    InProgress,
    /// Withdrawn during manual handling.
    Cancelled,
    /// Waiting for a Dune team member.
    NeedsManualReview,
}

/// One contract to submit for decoding with [`submit_contracts`](crate::client::DuneClient::submit_contracts).
///
/// Mirrors the form at <https://dune.com/contracts/new>. Build with `..Default::default()` for the
/// optional fields.
#[derive(Serialize, Debug, Clone, Default)]
pub struct ContractSubmissionInput {
    /// Chain the contract is deployed on, e.g. `ethereum`, `base`.
    pub blockchain_name: String,
    /// Contract address (hex for EVM chains).
    pub address: String,
    /// Project (namespace) the decoded tables are grouped under.
    pub project_name: String,
    /// Contract name used in the decoded table names.
    pub contract_name: String,
    /// The ABI, either as its JSON array of fragments or as a JSON string containing it.
    pub abi: serde_json::Value,
    /// The contract is a dynamic contract with several instances sharing one ABI.
    pub has_multiple_instances: bool,
    /// The instances are created by a factory contract.
    pub is_created_by_factory: bool,
    /// The ABI was written or edited by hand rather than fetched from an explorer.
    pub is_manual_abi: bool,
    /// The address is a proxy; the ABI belongs to its implementation.
    pub is_proxy: bool,
    /// Defaults to [`ContractSubmissionType::New`] when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submission_type: Option<ContractSubmissionType>,
    /// Why the contract is being resubmitted. Required for `Delete` and `Other`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resubmission_reason: Option<String>,
    /// Current project name; required for `Rename`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_project_name: Option<String>,
    /// Current contract name; required for `Rename`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_contract_name: Option<String>,
    /// Client-chosen key, unique per account, that makes the item safe to retry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// Request body for [`submit_contracts`](crate::client::DuneClient::submit_contracts).
#[derive(Serialize, Debug, Clone)]
pub struct SubmitContractsRequest {
    /// Between 1 and 100 contracts to submit.
    pub submissions: Vec<ContractSubmissionInput>,
}

/// Per-item outcome of a submission batch, matched to the request by `index`.
#[derive(Deserialize, Debug, Clone)]
pub struct ContractSubmissionResult {
    /// Position of the corresponding item in the request.
    pub index: u32,
    /// Id of the submission; set on success.
    #[serde(default)]
    pub submission_id: Option<String>,
    /// `pending` on success.
    #[serde(default)]
    pub status: Option<String>,
    /// True when the idempotency key matched an earlier submission and nothing new was created.
    #[serde(default)]
    pub replayed: bool,
    /// Why the item was rejected; set on failure.
    #[serde(default)]
    pub error: Option<String>,
}

/// Response from [`submit_contracts`](crate::client::DuneClient::submit_contracts).
#[derive(Deserialize, Debug)]
pub struct SubmitContractsResponse {
    /// One result per submitted item, in request order.
    pub results: Vec<ContractSubmissionResult>,
}

/// Filters and paging for [`list_contract_submissions`](crate::client::DuneClient::list_contract_submissions).
///
/// All fields are optional; build with `..Default::default()`.
#[derive(Serialize, Debug, Clone, Default)]
pub struct ListContractSubmissionsRequest {
    /// Maximum number of submissions to return (default 50, max 250).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// `next_cursor` from a previous response, to fetch the next page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Filter by blockchain, e.g. `ethereum`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blockchain_name: Option<String>,
    /// Filter by contract address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Filter by project (namespace) name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    /// Filter by contract name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_name: Option<String>,
    /// Filter by status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ContractSubmissionStatus>,
}

/// A contract decoding submission as returned by
/// [`list_contract_submissions`](crate::client::DuneClient::list_contract_submissions).
#[derive(Deserialize, Debug, Clone)]
pub struct ContractSubmission {
    /// Submission id.
    pub id: String,
    /// Chain the contract is deployed on.
    pub blockchain_name: String,
    /// Contract address.
    pub address: String,
    /// Project (namespace) name.
    pub project_name: String,
    /// Contract name.
    pub contract_name: String,
    /// Current status.
    pub status: ContractSubmissionStatus,
    /// The kind of change requested.
    pub submission_type: ContractSubmissionType,
    /// Reviewer or system comment explaining the current status.
    #[serde(default)]
    pub comment: Option<String>,
    /// When the submission was created.
    #[serde(deserialize_with = "datetime_from_str")]
    pub created_at: DateTime<Utc>,
    /// When the submission last changed.
    #[serde(deserialize_with = "datetime_from_str")]
    pub updated_at: DateTime<Utc>,
    /// The idempotency key it was submitted with, if any.
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

/// Response from [`list_contract_submissions`](crate::client::DuneClient::list_contract_submissions).
#[derive(Deserialize, Debug)]
pub struct ListContractSubmissionsResponse {
    /// Submissions on this page, newest first.
    pub submissions: Vec<ContractSubmission>,
    /// Total number of submissions matching the filters across all pages.
    pub total: u32,
    /// Present when more results exist; pass it back as `cursor`.
    #[serde(default)]
    pub next_cursor: Option<String>,
}
