# duners

A Rust client for the [Dune Analytics API](https://dune.com/docs/api/). Execute queries, wait for completion, and deserialize results into your own types.

[![docs.rs](https://img.shields.io/docsrs/duners)](https://docs.rs/duners)
[![crates.io](https://img.shields.io/crates/v/duners)](https://crates.io/crates/duners)

## Installation

```bash
cargo add duners
```

You’ll need the **tokio** runtime (e.g. `tokio` with `rt-multi-thread` and `macros`).

## Quick start

1. **Get an API key** from [Dune → Settings → API](https://dune.com/settings/api).
2. **Set it** (or put it in a `.env` file as `DUNE_API_KEY=...`):

   ```bash
   export DUNE_API_KEY="your-api-key"
   ```

3. **Run a saved query** using `run_query` (execute → wait until done → return all results):

```rust
use duners::{DuneClient, DuneRequestError};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Row {
    symbol: String,
    max_price: f64,
}

#[tokio::main]
async fn main() -> Result<(), DuneRequestError> {
    let client = DuneClient::from_env();
    let result = client.run_query::<Row>(971694, None, None).await?;
    println!("{:?}", result.get_rows());
    Ok(())
}
```

The **query ID** (e.g. `971694`) is the number at the end of a Dune query URL: `https://dune.com/queries/971694`.

To execute repository-owned SQL without creating a saved query, use `run_sql`:

```rust
let result = client
    .run_sql::<MyRow>("SELECT 1 AS value", None, None)
    .await?;
```

For large result sets, `stream_query` and `stream_sql` yield each result page as it is fetched instead of buffering everything in memory:

```rust
use futures_util::StreamExt;

let pages = client
    .stream_sql::<MyRow>("SELECT 1 AS value", None, None)
    .await?;
let mut pages = std::pin::pin!(pages);
while let Some(page) = pages.next().await {
    println!("{:?}", page?.get_rows());
}
```

## Authentication

- **`DuneClient::new(api_key)`** — pass the API key directly.
- **`DuneClient::from_env()`** — reads `DUNE_API_KEY` from the environment. If a `.env` file exists in the current directory, it is loaded first.

## Parameterized queries

For saved queries that take parameters, pass a list of [`Parameter`](https://docs.rs/duners/latest/duners/parameters/struct.Parameter.html) as the second argument to `run_query` (or `execute_query`):

```rust
use duners::{DuneClient, Parameter};

let params = vec![
    Parameter::text("WalletAddress", "0x1234..."),
    Parameter::number("MinAmount", "100"),
    Parameter::list("Token", "ETH"),
];
let result = client.run_query::<MyRow>(QUERY_ID, Some(params), None).await?;
```

Parameter names must match the names defined in the query on Dune.

## Deserializing result rows

Define a struct whose fields match the query’s columns and derive `Deserialize`. You can use your own types; depending on the column type, the API returns numbers and dates either as JSON numbers or as **strings**, so use the helpers in [`parse_utils`](https://docs.rs/duners/latest/duners/parse_utils/index.html) to accept both:

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

## Error handling

All fallible methods return `Result<_, DuneRequestError>`. Use `?` to propagate. `DuneRequestError` implements `std::error::Error` and `Display`; variants are:

- **`DuneRequestError::Dune(msg)`** — API returned an error (e.g. invalid API key, query not found).
- **`DuneRequestError::Request(msg)`** — network/HTTP error (e.g. connection failed, timeout).

## Documentation

Full API reference: **[docs.rs/duners](https://docs.rs/duners/latest/duners/)**

## License

MIT OR Apache-2.0
