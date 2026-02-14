use chrono::{DateTime, Utc};

/// Dune supports four parameter types; all are sent to the API as JSON strings.
#[derive(Debug, PartialEq)]
enum ParameterType {
    Text,
    Number,
    Enum,
    Date,
}

/// A single query parameter for a [parameterized Dune query](https://dune.com/docs/api/api-reference/execute-queries/execute-query-id/).
///
/// The parameter **name** must match the name defined in the query on Dune (e.g. in the query editor).
/// Use the constructors [`Parameter::text`], [`Parameter::number`], [`Parameter::date`], and
/// [`Parameter::list`] to build parameters of the correct type.
///
/// # Example
///
/// ```rust,no_run
/// use duners::Parameter;
/// use chrono::Utc;
///
/// let params = vec![
///     Parameter::text("WalletAddress", "0x1234..."),
///     Parameter::number("MinAmount", "100"),
///     Parameter::list("Token", "ETH"),
///     Parameter::date("StartDate", Utc::now()),
/// ];
/// ```
#[derive(Debug, PartialEq)]
pub struct Parameter {
    /// Parameter name (must match the query’s parameter name on Dune).
    pub key: String,
    ptype: ParameterType,
    /// String value sent to the API.
    pub value: String,
}

impl Parameter {
    /// Builds a **date** parameter. The value is sent as `YYYY-MM-DD HH:MM:SS`.
    pub fn date(name: &str, value: DateTime<Utc>) -> Self {
        Parameter {
            key: String::from(name),
            ptype: ParameterType::Date,
            // Dune date precision is to the second.
            // YYYY-MM-DD HH:MM:SS
            value: value.to_string()[..19].parse().unwrap(),
        }
    }

    /// Builds a **text** parameter (e.g. addresses, hashes, plain strings).
    pub fn text(name: &str, value: &str) -> Self {
        Parameter {
            key: String::from(name),
            ptype: ParameterType::Text,
            value: String::from(value),
        }
    }

    /// Builds a **number** parameter. Pass the value as a string (e.g. `"42"` or `"3.14"`).
    pub fn number(name: &str, value: &str) -> Self {
        Parameter {
            key: String::from(name),
            ptype: ParameterType::Number,
            value: String::from(value),
        }
    }

    /// Builds a **list/enum** parameter (dropdown-style; value must match one of the query’s options).
    pub fn list(name: &str, value: &str) -> Self {
        Parameter {
            key: String::from(name),
            ptype: ParameterType::Enum,
            value: String::from(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_utils::date_parse;

    #[test]
    fn new_parameter() {
        assert_eq!(
            Parameter::text("MyText", "Hello!"),
            Parameter {
                key: "MyText".to_string(),
                ptype: ParameterType::Text,
                value: "Hello!".to_string(),
            }
        );
        assert_eq!(
            Parameter::list("MyEnum", "Item 1"),
            Parameter {
                key: "MyEnum".to_string(),
                ptype: ParameterType::Enum,
                value: "Item 1".to_string(),
            }
        );
        assert_eq!(
            Parameter::number("MyNumber", "3.14159"),
            Parameter {
                key: "MyNumber".to_string(),
                ptype: ParameterType::Number,
                value: "3.14159".to_string(),
            }
        );
        let date_str = "2022-01-01T01:02:03.123Z";
        assert_eq!(
            Parameter::date("MyDate", date_parse(date_str).unwrap()),
            Parameter {
                key: "MyDate".to_string(),
                ptype: ParameterType::Date,
                value: "2022-01-01 01:02:03".to_string(),
            }
        )
    }

    #[test]
    fn derived_debug() {
        assert_eq!(format!("{:?}", ParameterType::Date), "Date");
        assert_eq!(
            format!("{:?}", Parameter::number("MyNumber", "3.14159")),
            "Parameter { key: \"MyNumber\", ptype: Number, value: \"3.14159\" }"
        );
    }
}
