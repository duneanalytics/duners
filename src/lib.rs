//! # duners
//!
//! A Rust client for the [Dune Analytics API](https://dune.com/docs/api/). Execute queries, poll for
//! completion, and deserialize results into your own types.
//!
//! ## Quick start
//!
//! 1. **Get an API key** from [Dune](https://dune.com/settings/api) and set it:
//!    ```bash
//!    export DUNE_API_KEY="your-api-key"
//!    ```
//!
//! 2. **Add the dependency** and run a query:
//!
//! ```rust,no_run
//! use duners::{DuneClient, DuneRequestError};
//! use serde::Deserialize;
//!
//! #[derive(Deserialize, Debug)]
//! struct Row {
//!     symbol: String,
//!     max_price: f64,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), DuneRequestError> {
//!     let client = DuneClient::from_env();
//!     let result = client.refresh::<Row>(971694, None, None).await?;
//!     println!("{:?}", result.get_rows());
//!     Ok(())
//! }
//! ```
//!
//! ## What’s in this crate
//!
//! - **[`DuneClient`](client::DuneClient)** — Main entry point. Create with [`DuneClient::new`](client::DuneClient::new) or [`DuneClient::from_env`](client::DuneClient::from_env).
//! - **[`refresh`](client::DuneClient::refresh)** — Run a query and wait for results (execute → poll status → return rows).
//! - **Lower-level API** — [`execute_query`](client::DuneClient::execute_query), [`get_status`](client::DuneClient::get_status), [`get_results`](client::DuneClient::get_results), [`cancel_execution`](client::DuneClient::cancel_execution) for full control.
//! - **[`Parameter`](parameters::Parameter)** — Query parameters (text, number, date, list) for parameterized queries.
//! - **[`parse_utils`](parse_utils)** — Helpers for deserializing Dune’s JSON (e.g. dates and numbers that come as strings): [`datetime_from_str`](parse_utils::datetime_from_str), [`f64_from_str`](parse_utils::f64_from_str).
//! - **[`DuneRequestError`](error::DuneRequestError)** — All request and parsing errors.
//!
//! See the [README](https://github.com/bh2smith/duners) for more examples and details.

pub mod client;
pub mod error;
pub mod parameters;
pub mod parse_utils;
pub mod response;

// Re-export commonly used types for convenience and clearer docs.
pub use client::DuneClient;
pub use error::DuneRequestError;
pub use parameters::Parameter;
pub use response::{ExecutionStatus, GetResultResponse};
