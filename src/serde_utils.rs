use serde::{Deserialize, Deserializer};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn deserialize_timestamp_millis<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
where
    D: Deserializer<'de>,
{
    let millis = u64::deserialize(deserializer)?;
    let duration = std::time::Duration::from_millis(millis);
    Ok(UNIX_EPOCH + duration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct TestStruct {
        #[serde(deserialize_with = "deserialize_timestamp_millis")]
        timestamp: SystemTime,
    }

    #[test]
    fn test_deserialize_timestamp_millis() {
        let json = r#"{"timestamp": 1640995200000}"#;
        let result: TestStruct = serde_json::from_str(json).unwrap();

        let expected_duration = std::time::Duration::from_millis(1640995200000);
        let expected_time = UNIX_EPOCH + expected_duration;

        assert_eq!(result.timestamp, expected_time);
    }
}
