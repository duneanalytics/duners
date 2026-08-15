//! Utilities for parsing Dune API response fields.
//!
//! Depending on the column type, Dune returns numbers and dates either as JSON numbers or as
//! **strings**. Use the deserializer helpers here with `#[serde(deserialize_with = "...")]` so
//! your structs can use `f64`, `u64`, or `DateTime<Utc>` regardless of the wire representation.

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
/// Accepts `YYYY-MM-DD HH:MM:SS` or `YYYY-MM-DD HH:MM:SS.ffffff`, with an optional ` UTC` suffix.
pub fn dune_date(date_str: &str) -> Result<DateTime<Utc>, ParseError> {
    let date_str = date_str.strip_suffix(" UTC").unwrap_or(date_str);
    // Try with microseconds first
    date_string_parser(date_str, "%Y-%m-%d %H:%M:%S.%f")
        .or_else(|_| date_string_parser(date_str, "%Y-%m-%d %H:%M:%S"))
}

fn parse_datetime(date_str: &str) -> Result<DateTime<Utc>, ParseError> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(date_str) {
        return Ok(parsed.with_timezone(&Utc));
    }
    date_parse(date_str).or_else(|_| dune_date(date_str))
}

/// Serde deserializer for date/time fields that Dune returns as strings.
///
/// Accepts RFC 3339 (with any offset), the API metadata format (`2022-01-01T12:00:00.000Z`), and
/// query-result timestamps (`2022-01-01 12:00:00[.ffffff][ UTC]`). Use with
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
    parse_datetime(&s).map_err(de::Error::custom)
}

/// Serde deserializer for optional date/time strings (e.g. `expires_at`).
///
/// Accepts the same formats as [`datetime_from_str`]; `null` and missing values become `None`.
pub fn optional_datetime_from_str<'de, D>(
    deserializer: D,
) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Deserialize::deserialize(deserializer)?;
    s.map(|s| parse_datetime(&s).map_err(de::Error::custom))
        .transpose()
}

/// Serde deserializer for numeric fields that Dune returns as a JSON number **or** a string.
///
/// Dune encodes some numeric column types (e.g. decimals) as strings like `"1.25"` and others as
/// plain JSON numbers. Use with `#[serde(deserialize_with = "duners::parse_utils::f64_from_str")]`
/// on `f64` fields to accept both.
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
/// let row: MyRow = serde_json::from_str(r#"{"price": "1.25"}"#).unwrap();
/// assert_eq!(row.price, 1.25);
/// let row: MyRow = serde_json::from_str(r#"{"price": 1.25}"#).unwrap();
/// assert_eq!(row.price, 1.25);
/// ```
pub fn f64_from_str<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| de::Error::custom("number does not fit in f64")),
        Value::String(s) => s.parse().map_err(de::Error::custom),
        other => Err(de::Error::custom(format!(
            "expected a number or string, got {other}"
        ))),
    }
}

/// Serde deserializer for unsigned integer fields that Dune returns as a JSON number **or** a
/// string.
///
/// Dune encodes `bigint`/`uint256` columns as strings like `"12345"` when they may exceed the
/// JSON number range. Use with `#[serde(deserialize_with = "duners::parse_utils::u64_from_str")]`
/// on `u64` fields to accept both.
///
/// # Example
///
/// ```rust
/// use duners::parse_utils::u64_from_str;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct MyRow {
///     #[serde(deserialize_with = "u64_from_str")]
///     trade_count: u64,
/// }
///
/// let row: MyRow = serde_json::from_str(r#"{"trade_count": "42"}"#).unwrap();
/// assert_eq!(row.trade_count, 42);
/// let row: MyRow = serde_json::from_str(r#"{"trade_count": 42}"#).unwrap();
/// assert_eq!(row.trade_count, 42);
/// ```
pub fn u64_from_str<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| de::Error::custom("number is not an unsigned integer")),
        Value::String(s) => s.parse().map_err(de::Error::custom),
        other => Err(de::Error::custom(format!(
            "expected a number or string, got {other}"
        ))),
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

    #[test]
    fn dune_date_with_utc_suffix() {
        let date_str = "2022-05-04 00:00:00.000 UTC";
        assert_eq!(
            dune_date(date_str).unwrap().to_string(),
            "2022-05-04 00:00:00 UTC"
        )
    }

    #[test]
    fn parse_datetime_accepts_all_formats() {
        for date_str in [
            "2022-01-01T01:02:03+00:00",
            "2022-01-01T02:02:03+01:00",
            "2022-01-01T01:02:03Z",
            "2022-01-01T01:02:03.000Z",
            "2022-01-01 01:02:03",
            "2022-01-01 01:02:03.000",
            "2022-01-01 01:02:03 UTC",
        ] {
            assert_eq!(
                parse_datetime(date_str).unwrap().to_string(),
                "2022-01-01 01:02:03 UTC",
                "failed for {date_str}"
            );
        }
    }

    #[derive(Deserialize)]
    struct NumberRow {
        #[serde(deserialize_with = "f64_from_str")]
        float: f64,
        #[serde(deserialize_with = "u64_from_str")]
        int: u64,
        #[serde(default, deserialize_with = "optional_datetime_from_str")]
        date: Option<DateTime<Utc>>,
    }

    #[test]
    fn numbers_from_string_or_number() {
        let row: NumberRow =
            serde_json::from_str(r#"{"float": "1.25", "int": "42", "date": null}"#).unwrap();
        assert_eq!(row.float, 1.25);
        assert_eq!(row.int, 42);
        assert_eq!(row.date, None);

        let row: NumberRow =
            serde_json::from_str(r#"{"float": 1.25, "int": 42, "date": "2022-01-01 01:02:03"}"#)
                .unwrap();
        assert_eq!(row.float, 1.25);
        assert_eq!(row.int, 42);
        assert_eq!(row.date.unwrap().to_string(), "2022-01-01 01:02:03 UTC");
    }

    #[test]
    fn numbers_reject_invalid_values() {
        assert!(serde_json::from_str::<NumberRow>(r#"{"float": true, "int": 1}"#).is_err());
        assert!(serde_json::from_str::<NumberRow>(r#"{"float": 1.0, "int": -1}"#).is_err());
        assert!(serde_json::from_str::<NumberRow>(r#"{"float": 1.0, "int": 1.5}"#).is_err());
        assert!(serde_json::from_str::<NumberRow>(r#"{"float": "abc", "int": 1}"#).is_err());
    }
}
