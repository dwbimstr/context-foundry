//! Strict JSON: one value read through serde_json's own parser, with a
//! duplicate object key refused at any depth and nesting bounded, then the
//! typed struct. The depth bound and the error constructor stay in the
//! parent module.
use super::*;

// ---------------------------------------------------------------------------
// Strict JSON
// ---------------------------------------------------------------------------

/// One JSON value read through serde_json's own parser, so malformed syntax
/// fails at once. A duplicate object key at ANY depth is refused (serde
/// would otherwise keep the last one silently) and nesting is bounded.
struct StrictValue {
    depth: usize,
}

impl StrictValue {
    fn nested<E: serde::de::Error>(&self) -> Result<StrictValue, E> {
        if self.depth >= JSON_MAX_DEPTH {
            return Err(E::custom("JSON nests too deep"));
        }
        Ok(StrictValue {
            depth: self.depth + 1,
        })
    }
}

impl<'de> serde::de::DeserializeSeed<'de> for StrictValue {
    type Value = serde_json::Value;

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> serde::de::Visitor<'de> for StrictValue {
    type Value = serde_json::Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(value.into())
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(value.into())
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(value.into())
    }

    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
        serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| E::custom("JSON number is not finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(value.into())
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(value.into())
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(self.nested::<A::Error>()?)? {
            items.push(item);
        }
        Ok(serde_json::Value::Array(items))
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        use serde::de::Error as _;
        let mut object = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if object.contains_key(&key) {
                return Err(A::Error::custom(format!("duplicate JSON key {key:?}")));
            }
            let value = map.next_value_seed(self.nested::<A::Error>()?)?;
            object.insert(key, value);
        }
        Ok(serde_json::Value::Object(object))
    }
}

/// Parse `bytes` strictly into `T`: serde_json's parser through
/// [`StrictValue`] (no duplicate keys, bounded depth, nothing trailing),
/// then the typed struct, whose `deny_unknown_fields` and non-optional
/// fields refuse unknown and null members. Every failure is `code`.
pub(crate) fn strict_json<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    code: &'static str,
    what: &str,
) -> FResult<T> {
    use serde::de::DeserializeSeed as _;
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = StrictValue { depth: 0 }
        .deserialize(&mut deserializer)
        .and_then(|value| deserializer.end().map(|()| value))
        .map_err(|e| fail(code, format!("{what} is not strict JSON: {e}")))?;
    serde_json::from_value(value).map_err(|e| fail(code, format!("{what}: {e}")))
}
