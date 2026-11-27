# duners

A Rust client for the [Dune Analytics API](https://dune.com/docs/api/). Execute queries, wait for completion, and deserialize results into your own types.

[![docs.rs](https://img.shields.io/docsrs/duners)](https://docs.rs/duners)
[![crates.io](https://img.shields.io/crates/v/duners)](https://crates.io/crates/duners)

## Installation

```bash
cargo new dune-example
cd dune-example
cargo add duners
cargo add serde --features derive
cargo add tokio --features macros,rt-multi-thread
```

The example below uses `serde` to deserialize rows and `tokio` to run async code. Add both as direct dependencies, even though `duners` also depends on them.

## Quick start

1. **Get an API key** from [Dune → Settings → API](https://dune.com/settings/api).
2. **Set it** (or put it in a `.env` file as `DUNE_API_KEY=...`):

   ```bash
   export DUNE_API_KEY="your-api-key"
   ```

3. **Run a query** by putting this in `src/main.rs` and running `cargo run`:

```rust
use duners::{DuneClient, DuneRequestError};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct ValueRow {
    value: u64,
}

#[tokio::main]
async fn main() -> Result<(), DuneRequestError> {
    let client = DuneClient::from_env();
    let result = client.run_sql::<ValueRow>("SELECT 1 AS value", None, None).await?;
    println!("{:?}", result.get_rows());
    Ok(())
}
```

`run_sql` submits SQL, waits for completion, and returns all result rows. The rest of the examples assume an async function and a `client` created as above. Executing a query consumes credits.

To run a **saved query**, use its ID (the number at the end of a Dune query URL). For example, [query 971694](https://dune.com/queries/971694) returns `symbol` and `max_price`:

```rust
#[derive(Deserialize, Debug)]
struct PriceRow {
    symbol: String,
    max_price: f64,
}

let result = client.run_query::<PriceRow>(971694, None, None).await?;
println!("{:?}", result.get_rows());
```

For large result sets, `stream_query` and `stream_sql` yield each result page as it is fetched instead of buffering everything in memory:

```bash
cargo add futures-util
```

```rust
use futures_util::StreamExt;

let pages = client
    .stream_sql::<ValueRow>("SELECT 1 AS value", None, None)
    .await?;
let mut pages = std::pin::pin!(pages);
while let Some(page) = pages.next().await {
    println!("{:?}", page?.get_rows());
}
```

## Reuse the latest results

If a saved query has already run, fetch its latest result without starting another execution. This reads one API result page; use `run_query` or `stream_query` when you need every page from a new execution. Reading result rows can still consume credits.

```rust
let result = client.get_latest_results::<PriceRow>(971694).await?;
println!("{:?}", result.get_rows());

let csv = client.get_latest_results_csv(971694).await?;
println!("{csv}");
```

## Authentication

- **`DuneClient::new(api_key)`** — pass the API key directly.
- **`DuneClient::from_env()`** — reads `DUNE_API_KEY` from the environment. If a `.env` file exists in the current directory, it is loaded first.

## Parameterized queries

For saved queries that take parameters, pass a list of [`Parameter`](https://docs.rs/duners/latest/duners/parameters/struct.Parameter.html) as the second argument to `run_query` (or `execute_query`). Query 971694 has an `EnumParam` list parameter:

```rust
use duners::Parameter;

let params = vec![Parameter::list("EnumParam", "WETH")];
let result = client.run_query::<PriceRow>(971694, Some(params), None).await?;
```

Parameter names must match the names defined in the query on Dune.

## Manage saved queries

Create a query, read it back, update its description, then run it. Creating and updating queries requires an API key with write access and an eligible plan. Use your own query ID for subsequent calls.

```rust
use duners::QueryBody;

let created = client.create_query(QueryBody {
    name: Some("SDK example".into()),
    query_sql: Some("SELECT 1 AS value".into()),
    ..Default::default()
}).await?;
let query = client.get_query(created.query_id).await?;
println!("{}: {}", query.name, query.query_sql);

client.update_query(created.query_id, QueryBody {
    description: Some("A first query from Rust".into()),
    ..Default::default()
}).await?;
let result = client.run_query::<ValueRow>(created.query_id, None, None).await?;
println!("{:?}", result.get_rows());
```

## Upload tables

For a one-off CSV upload, Dune infers the schema. Uploading to an existing table name replaces its data, so choose the name carefully. Uploads require write access and consume credits.

```rust
use duners::UploadCsvRequest;

let table = client.upload_csv(UploadCsvRequest {
    data: "symbol,price\nETH,123.0\n".into(),
    table_name: "sdk_prices".into(),
    description: None,
    is_private: Some(true),
}).await?;
println!("{}", table.full_name);
```

To append rows, create a table with an explicit schema, then insert data. Replace `your_team` with your account or team namespace. The CSV header must match the table's columns.

```rust
use duners::{ColumnDef, CreateTableRequest};

let namespace = "your_team";
let table_name = "sdk_prices_incremental";
client.create_table(CreateTableRequest {
    namespace: namespace.into(),
    table_name: table_name.into(),
    schema: vec![
        ColumnDef { name: "symbol".into(), column_type: "varchar".into(), nullable: None },
        ColumnDef { name: "price".into(), column_type: "double".into(), nullable: None },
    ],
    description: None,
    is_private: Some(true),
}).await?;
client.insert_table_rows(
    namespace, table_name, "text/csv", "symbol,price\nETH,123.0\n".into(),
).await?;
```

`clear_table(namespace, table_name)` removes every row but preserves the schema. `delete_table(namespace, table_name)` permanently removes the table and its data.

## Deserializing result rows

Define a struct whose fields match the query’s columns and derive `Deserialize`. You can use your own types; depending on the column type, the API returns numbers and dates either as JSON numbers or as **strings**, so use the helpers in [`parse_utils`](https://docs.rs/duners/latest/duners/parse_utils/index.html) to accept both:

```bash
cargo add chrono --features serde
```

```rust
use chrono::{DateTime, Utc};
use duners::parse_utils::{datetime_from_str, f64_from_str, optional_datetime_from_str, u64_from_str};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct ResultStruct {
    text_field: String,
    #[serde(deserialize_with = "f64_from_str")]
    volume_usd: f64,
    #[serde(deserialize_with = "u64_from_str")]
    trade_count: u64,
    #[serde(deserialize_with = "datetime_from_str")]
    block_time: DateTime<Utc>,
    #[serde(default, deserialize_with = "optional_datetime_from_str")]
    first_trade_at: Option<DateTime<Utc>>,
}
```

- **`f64_from_str`** / **`u64_from_str`** — for numeric columns, whether they arrive as JSON numbers or strings (Dune encodes e.g. decimals and bigints as strings).
- **`datetime_from_str`** / **`optional_datetime_from_str`** — for date/timestamp columns; accepts RFC 3339 as well as Dune result formats like `2022-01-01 01:02:03[.000][ UTC]`.

## Lower-level API

For more control (e.g. custom polling or cancellation):

- **`execute_query(query_id, params)`** — start execution; returns an `execution_id`.
- **`get_status(execution_id)`** — check status (`Complete`, `Executing`, `Pending`, `Cancelled`, `Failed`).
- **`get_results(execution_id)`** — fetch result rows (only valid when status is `Complete`).
- **`cancel_execution(execution_id)`** — cancel a running execution.

See the [API docs](https://docs.rs/duners) for details and types.

## Contract decoding

Submit contracts for decoding in batches and track their status. Submissions are attributed to the user who created the API key; see the [docs](https://docs.dune.com/api-reference/contracts/introduction) for plan requirements.

```bash
cargo add serde_json
```

```rust,no_run
use duners::{ContractSubmissionInput, DuneClient, ListContractSubmissionsRequest, SubmitContractsRequest};

# async fn run() -> Result<(), duners::DuneRequestError> {
let client = DuneClient::from_env();
let resp = client.submit_contracts(SubmitContractsRequest {
    submissions: vec![ContractSubmissionInput {
        blockchain_name: "ethereum".into(),
        address: "0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984".into(),
        project_name: "uniswap".into(),
        contract_name: "UniswapToken".into(),
        abi: serde_json::json!([{"type": "event", "name": "Transfer", "inputs": []}]),
        idempotency_key: Some("uniswap-token/ethereum/1".into()), // optional, makes retries safe
        ..Default::default()
    }],
}).await?;
// One result per submission, matched by index: submission_id + status "pending", or error.
println!("{:?}", resp.results);

let page = client.list_contract_submissions(ListContractSubmissionsRequest {
    limit: Some(20),
    ..Default::default()
}).await?;
// Pass page.next_cursor back as `cursor` to fetch the next page.
println!("{} of {}", page.submissions.len(), page.total);
# Ok(()) }
```

## Error handling

All fallible methods return `Result<_, DuneRequestError>`. Use `?` to propagate. `DuneRequestError` implements `std::error::Error` and `Display`; variants are:

- **`DuneRequestError::Dune(msg)`** — API returned an error (e.g. invalid API key, query not found).
- **`DuneRequestError::Request(msg)`** — network/HTTP error (e.g. connection failed, timeout).

## Documentation

Full API reference: **[docs.rs/duners](https://docs.rs/duners/latest/duners/)**

## License

MIT OR Apache-2.0
