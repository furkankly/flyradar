use std::time::Duration;

use humantime_serde::re::humantime::parse_duration;
use serde::de::{self, Visitor};
use serde::{Deserializer, Serializer};

pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    struct DurationVisitor;

    impl Visitor<'_> for DurationVisitor {
        type Value = Duration;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a duration string, integer (nanoseconds), or null")
        }

        fn visit_none<E: de::Error>(self) -> Result<Duration, E> {
            Ok(Duration::from_secs(0))
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<Duration, E> {
            Ok(Duration::from_nanos(value))
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<Duration, E> {
            if value < 0 {
                return Err(E::custom("negative duration"));
            }
            Ok(Duration::from_nanos(value as u64))
        }

        fn visit_f64<E: de::Error>(self, value: f64) -> Result<Duration, E> {
            if value < 0.0 {
                return Err(E::custom("negative duration"));
            }
            Ok(Duration::from_nanos(value as u64))
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Duration, E> {
            parse_duration(value).map_err(E::custom)
        }

        fn visit_string<E: de::Error>(self, value: String) -> Result<Duration, E> {
            self.visit_str(&value)
        }
    }

    deserializer.deserialize_any(DurationVisitor)
}

pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    humantime_serde::serialize(duration, serializer)
}
