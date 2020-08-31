//! Extensions for `serde_json::Value`.

use serde_json::{Number, Value};
use std::hash::{Hash, Hasher};
use std::ops::Deref;

/// The funny way I have to get a f64 from a `Number`. This is a lossy conversion.
pub fn force_f64(num: &Number) -> f64 {
    num.as_f64()
        .or_else(|| num.as_i64().map(|num| num as f64))
        .or_else(|| num.as_u64().map(|num| num as f64))
        .expect("all cases covered")
}

/// Hashes a JSON value reference
#[derive(Debug, Eq)]
pub(crate) struct HashableJsonRef<V: Deref<Target = Value>>(pub V);

impl<V: Deref<Target = Value>> PartialEq for HashableJsonRef<V>
where
    V: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<V: Deref<Target = Value>> Hash for HashableJsonRef<V> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        match self.0.deref() {
            Value::Null => {
                0i8.hash(hasher);
            }
            Value::Bool(b) => {
                1i8.hash(hasher);
                b.hash(hasher);
            }
            Value::Number(num) => {
                2i8.hash(hasher);
                num.as_f64()
                    .map(|num| num.to_ne_bytes().hash(hasher))
                    .or_else(|| num.as_i64().map(|num| num.hash(hasher)))
                    .or_else(|| num.as_u64().map(|num| num.hash(hasher)))
                    .expect("all cases covered");
            }
            Value::String(s) => {
                3i8.hash(hasher);
                s.hash(hasher);
            }
            Value::Array(array) => {
                4i8.hash(hasher);
                for value in array {
                    HashableJsonRef(value).hash(hasher);
                }
            }
            Value::Object(obj) => {
                5i8.hash(hasher);
                for (key, value) in obj {
                    key.hash(hasher);
                    HashableJsonRef(value).hash(hasher);
                }
            }
        }
    }
}

/// Hashes a JSON value
#[derive(Debug, Eq)]
pub(crate) struct HashableJson(pub Value);

impl PartialEq for HashableJson {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Hash for HashableJson {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        HashableJsonRef(&self.0).hash(hasher)
    }
}
