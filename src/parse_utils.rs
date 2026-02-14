//! Utilities for parsing Dune API response fields.
//!
//! Dune often returns numbers and dates as **strings** in JSON. Use the deserializer helpers here
//! with `#[serde(deserialize_with = "...")]` so your structs can use `f64` or `DateTime<Utc>`.

use chrono::{DateTime, NaiveDateTime, ParseError, Utc};
use serde::{de, Deserialize, Deserializer};
use serde_json::Value;

fn date_string_parser(date_str: &str, format: &str) -> Result<DateTime<Utc>, ParseError> {
    let native = NaiveDateTime::parse_from_str(date_str, format);
    Ok(DateTime::from_naive_utc_and_offset(native?, Utc))
}

/// Parses API metadata date strings (e.g. `submitted_at`, `execution_ended_at`).
///
/// Format: `%Y-%m-%dT%H:%M:%S.%fZ` (ISO 8601 with optional subseconds).
///
/// # Example
///
/// ```rust
/// use duners::parse_utils::date_parse;
///
/// let dt = date_parse("2022-01-01T12:00:00.000Z").unwrap();
/// assert_eq!(dt.format("%Y-%m-%d").to_string(), "2022-01-01");
/// ```
pub fn date_parse(date_str: &str) -> Result<DateTime<Utc>, ParseError> {
    date_string_parser(date_str, "%Y-%m-%dT%H:%M:%S.%fZ")
}

/// Parses timestamp strings returned in **query result** columns (Dune timestamp type).
///
/// Accepts `YYYY-MM-DD HH:MM:SS` or `YYYY-MM-DD HH:MM:SS.ffffff`.
pub fn dune_date(date_str: &str) -> Result<DateTime<Utc>, ParseError> {
    // Try with microseconds first
    date_string_parser(date_str, "%Y-%m-%d %H:%M:%S.%f")
        .or_else(|_| date_string_parser(date_str, "%Y-%m-%d %H:%M:%S"))
}

/// Serde deserializer for date/time fields that Dune returns as strings.
///
/// Tries API metadata format first, then query-result timestamp format. Use with
/// `#[serde(deserialize_with = "duners::parse_utils::datetime_from_str")]` on `DateTime<Utc>` fields.
///
/// # Example
///
/// ```ignore
/// #[derive(Deserialize)]
/// struct MyRow {
///     #[serde(deserialize_with = "duners::parse_utils::datetime_from_str")]
///     created_at: DateTime<Utc>,
/// }
/// ```
pub fn datetime_from_str<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    match date_parse(&s) {
        // First try to parse response type date strings
        Ok(parsed_date) => Ok(parsed_date),
        Err(_) => {
            // First attempt didn't work, try another format
            dune_date(&s).map_err(de::Error::custom)
        }
    }
}

/// Serde deserializer for optional date/time strings (e.g. `expires_at`).
pub fn optional_datetime_from_str<'de, D>(
    deserializer: D,
) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Deserialize::deserialize(deserializer)?;
    match s {
        None => Ok(None),
        Some(s) => {
            let date = date_parse(&s).map_err(de::Error::custom)?;
            Ok(Some(date))
        }
    }
}

/// Serde deserializer for numeric fields that Dune returns as strings.
///
/// Use with `#[serde(deserialize_with = "duners::parse_utils::f64_from_str")]` on `f64` fields
/// when the API returns a string like `"3.14"` instead of a number.
///
/// # Example
///
/// ```rust
/// use duners::parse_utils::f64_from_str;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct MyRow {
///     #[serde(deserialize_with = "f64_from_str")]
///     price: f64,
/// }
///
/// // In real usage, MyRow is deserialized from Dune API JSON.
/// ```
pub fn f64_from_str<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Value = Deserialize::deserialize(deserializer)?;
    if let Value::String(s) = value {
        s.parse().map_err(de::Error::custom)
    } else {
        Err(de::Error::custom("Expected a string"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_parse_works() {
        let date_str = "2022-01-01T01:02:03.123Z";
        assert_eq!(
            date_parse(date_str).unwrap().to_string(),
            "2022-01-01 01:02:03.000000123 UTC"
        )
    }

    #[test]
    fn new_dune_date() {
        let date_str = "2022-05-04 00:00:00.000";
        assert_eq!(
            dune_date(date_str).unwrap().to_string(),
            "2022-05-04 00:00:00 UTC"
        )
    }

    #[test]
    fn dune_date_without_microseconds() {
        let date_str = "2022-05-04 00:00:00";
        assert_eq!(
            dune_date(date_str).unwrap().to_string(),
            "2022-05-04 00:00:00 UTC"
        )
    }
}
