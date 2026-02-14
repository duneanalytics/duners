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

3. **Run a query** using the `refresh` helper (execute → wait until done → return results):

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
    let result = client.refresh::<Row>(971694, None, None).await?;
    println!("{:?}", result.get_rows());
    Ok(())
}
```

The **query ID** (e.g. `971694`) is the number at the end of a Dune query URL: `https://dune.com/queries/971694`.

## Authentication

- **`DuneClient::new(api_key)`** — pass the API key directly.
- **`DuneClient::from_env()`** — reads `DUNE_API_KEY` from the environment. If a `.env` file exists in the current directory, it is loaded first.

## Parameterized queries

For queries that take parameters, pass a list of [`Parameter`](https://docs.rs/duners/latest/duners/parameters/struct.Parameter.html) as the second argument to `refresh` (or `execute_query`):

```rust
use duners::{DuneClient, Parameter};

let params = vec![
    Parameter::text("WalletAddress", "0x1234..."),
    Parameter::number("MinAmount", "100"),
    Parameter::list("Token", "ETH"),
];
let result = client.refresh::<MyRow>(QUERY_ID, Some(params), None).await?;
```

Parameter names must match the names defined in the query on Dune.

## Deserializing result rows

Define a struct whose fields match the query’s columns and derive `Deserialize`. You can use your own types; the API often returns numbers and dates as **strings**, so use the helpers in [`parse_utils`](https://docs.rs/duners/latest/duners/parse_utils/index.html) when needed:

```rust
use chrono::{DateTime, Utc};
use duners::parse_utils::{datetime_from_str, f64_from_str};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct ResultStruct {
    text_field: String,
    #[serde(deserialize_with = "f64_from_str")]
    number_field: f64,
    #[serde(deserialize_with = "datetime_from_str")]
    date_field: DateTime<Utc>,
    list_field: String,
}
```

- **`f64_from_str`** — for numeric columns that come as strings.
- **`datetime_from_str`** — for date/timestamp columns that come as strings.

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
