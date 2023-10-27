use candid::CandidType;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, CandidType, Serialize, Deserialize, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("Key '{0}' not found")]
    KeyNotFound(String),
    #[error("'{0}' is not an object")]
    NotAnObject(String),
}

pub trait ValueParser {
    fn parse(&self, dot_path: &str) -> Result<&Value, ParseError>;
}

impl ValueParser for Value {
    fn parse(&self, dot_path: &str) -> Result<&Value, ParseError> {
        let mut current_value = self;

        for key in dot_path.split('.') {
            match current_value {
                Value::Object(map) => {
                    current_value = map
                        .get(key)
                        .ok_or(ParseError::KeyNotFound(key.to_string()))?;
                }
                _ => return Err(ParseError::NotAnObject(key.to_string())),
            }
        }

        Ok(current_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_parser() {
        // Sample JSON data for testing
        let data = r#"
        {
            "price": 100
        }
        "#;

        let parsed_data: Value = serde_json::from_str(data).unwrap();

        // Testing various dot notations
        let dot_notations = "price";

        assert_eq!(
            parsed_data.parse(dot_notations).unwrap(),
            &Value::Number(100.into())
        )
    }

    #[test]
    fn test_nested_parser() {
        // Sample JSON data for testing
        let data = r#"
        {
            "price": {
                "value": 100
            }
        }
        "#;

        let parsed_data: Value = serde_json::from_str(data).unwrap();

        // Testing various dot notations
        let dot_notations = "price.value";

        assert_eq!(
            parsed_data.parse(dot_notations).unwrap(),
            &Value::Number(100.into())
        )
    }

    #[test]
    fn test_array_parser() {
        // Sample JSON data for testing
        let data = r#"
        {
            "price": {
                "data": {
                    "value": 800
                }
            }
        }
        "#;

        let parsed_data: Value = serde_json::from_str(data).unwrap();

        // Testing various dot notations
        let dot_notations = "price.data.value";

        assert_eq!(
            parsed_data.parse(dot_notations).unwrap(),
            &Value::Number(800.into())
        )
    }
}
