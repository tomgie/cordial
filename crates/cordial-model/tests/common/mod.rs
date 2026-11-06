//! Helpers for fixture tests.
//!
//! A fixture test decodes a JSON example from the Discord docs, encodes it again, and
//! compares the two. A difference shows a field that the model does not keep.

#![allow(dead_code, unreachable_pub)]

use std::path::PathBuf;

use cordial_model::Timestamp;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Reads a fixture file from `fixtures/docs/<path>.json`.
pub fn read_fixture(path: &str) -> Value {
    let full_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/docs")
        .join(format!("{path}.json"));
    let text = std::fs::read_to_string(&full_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", full_path.display()));
    serde_json::from_str(&text).unwrap()
}

/// Decodes a fixture into `T`, encodes it again, and checks that no data is lost.
pub fn assert_round_trip<T: DeserializeOwned + Serialize>(path: &str) -> T {
    let input = read_fixture(path);
    assert_value_round_trip::<T>(path, &input)
}

/// Decodes a JSON value into `T`, encodes it again, and checks that no data is lost.
pub fn assert_value_round_trip<T: DeserializeOwned + Serialize>(name: &str, input: &Value) -> T {
    let decoded: T = serde_json::from_value(input.clone())
        .unwrap_or_else(|error| panic!("{name}: cannot decode: {error}"));
    let output = serde_json::to_value(&decoded).unwrap();

    let mut differences = Vec::new();
    compare(&mut differences, "$", input, &output);
    assert!(
        differences.is_empty(),
        "{name}: the round trip lost or changed data:\n  {}",
        differences.join("\n  ")
    );

    decoded
}

fn compare(differences: &mut Vec<String>, path: &str, input: &Value, output: &Value) {
    match (input, output) {
        (Value::Object(input_map), Value::Object(output_map)) => {
            for (key, input_value) in input_map {
                let child = format!("{path}.{key}");
                match output_map.get(key) {
                    Some(output_value) => compare(differences, &child, input_value, output_value),
                    None if is_empty_value(input_value) => {}
                    None => differences.push(format!("{child}: missing in output")),
                }
            }
            for (key, output_value) in output_map {
                if !input_map.contains_key(key) && !is_empty_value(output_value) {
                    differences.push(format!("{path}.{key}: added in output as {output_value}"));
                }
            }
        }
        (Value::Array(input_items), Value::Array(output_items)) => {
            if input_items.len() != output_items.len() {
                differences.push(format!(
                    "{path}: array length {} became {}",
                    input_items.len(),
                    output_items.len()
                ));
                return;
            }
            for (index, (input_item, output_item)) in
                input_items.iter().zip(output_items).enumerate()
            {
                compare(
                    differences,
                    &format!("{path}[{index}]"),
                    input_item,
                    output_item,
                );
            }
        }
        (Value::Number(input_number), Value::Number(output_number)) => {
            if input_number.as_f64() != output_number.as_f64() {
                differences.push(format!("{path}: {input_number} became {output_number}"));
            }
        }
        (Value::String(input_text), Value::String(output_text)) => {
            if input_text != output_text && !same_timestamp(input_text, output_text) {
                differences.push(format!("{path}: {input_text:?} became {output_text:?}"));
            }
        }
        _ if input != output => {
            differences.push(format!("{path}: {input} became {output}"));
        }
        _ => {}
    }
}

/// Tells if a value carries no data, so that its addition in the output is acceptable.
fn is_empty_value(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(false) => true,
        Value::Number(number) => number.as_f64() == Some(0.0),
        Value::String(text) => text.is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(map) => map.is_empty(),
        Value::Bool(true) => false,
    }
}

fn same_timestamp(input: &str, output: &str) -> bool {
    match (Timestamp::parse(input), Timestamp::parse(output)) {
        (Ok(input_time), Ok(output_time)) => input_time == output_time,
        _ => false,
    }
}
