#![allow(unused_crate_dependencies)]

use chrono::{DateTime, Utc};
use duners::{
    client::DuneClient, parameters::Parameter, parse_utils::datetime_from_str,
    parse_utils::f64_from_str,
};
use serde::Deserialize;

// User must declare the expected query return fields and types!
#[derive(Deserialize, Debug, PartialEq)]
struct ResultStruct {
    text_field: String,
    #[serde(deserialize_with = "f64_from_str")]
    number_field: f64,
    #[serde(deserialize_with = "datetime_from_str")]
    date_field: DateTime<Utc>,
    list_field: String,
}

#[tokio::test]
async fn test_external_use() {
    let dune = DuneClient::from_env();
    let results = dune
        .refresh::<ResultStruct>(1215383, None, None)
        .await
        .unwrap();
    println!("{:?}", results.get_rows());
}

#[tokio::test]
async fn test_blocks() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Block {
        pub number: u64,
        pub time: u64,
    }

    let dune = DuneClient::from_env();
    let (start, end) = (5, 7);
    let result = dune
        .refresh::<Block>(
            3238189,
            Some(vec![
                Parameter::number("Start", &start.to_string()),
                Parameter::number("Width", &(end - start).to_string()),
            ]),
            Some(1),
        )
        .await
        .unwrap();
    println!("{:?}", result.execution_id);
    println!("{:?}", result.result.metadata);
    assert_eq!(
        result.get_rows(),
        vec![
            Block {
                number: 5,
                time: 1438270083
            },
            Block {
                number: 6,
                time: 1438270107
            },
            Block {
                number: 7,
                time: 1438270110
            }
        ]
    )
}
