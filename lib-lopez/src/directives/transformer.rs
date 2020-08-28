use regex::{Captures, Regex};
use serde_json::{Map, Value, Number};
use std::fmt;

/// Puts captures into a nice JSON.
fn capture_json(regex: &Regex, captures: Captures) -> Map<String, Value> {
    captures
        .iter()
        .zip(regex.capture_names())
        .enumerate()
        .filter_map(|(i, (maybe_capture, maybe_name))| {
            maybe_capture.map(|capture| {
                (
                    maybe_name
                        .map(|name| name.to_owned())
                        .unwrap_or_else(|| i.to_string()),
                    capture.as_str().to_owned().into(),
                )
            })
        })
        .collect::<Map<String, Value>>()
}

/// The funny way I have to get a f64 from a `Number`. This is a lossy conversion.
fn force_f64(num: &Number) -> f64 {
    num.as_f64()
        .or_else(|| num.as_i64().map(|num| num as f64))
        .or_else(|| num.as_u64().map(|num| num as f64))
        .expect("all cases covered")
}

#[derive(Debug, Clone)]
pub enum Type {
    Any,
    Bool,
    Number,
    String,
    Array(Box<Type>),
    Map(Box<Type>),
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Any => write!(f, "any"),
            Type::Bool => write!(f, "bool"),
            Type::Number => write!(f, "number"),
            Type::String => write!(f, "string"),
            Type::Array(typ) => write!(f, "array[{}]", typ),
            Type::Map(typ) => write!(f, "map[string, {}]", typ),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Transformer {
    // General purpose:
    IsNull,
    IsNotNull,
    Hash,

    // Numeric:
    AsNumber,
    GreaterThan(f64),
    LesserThan(f64),
    Equals(f64),

    // Collections:
    Length,
    IsEmpty,
    Get(String),
    GetIdx(usize),
    Flatten,
    Each(Box<Transformer>),
    Filter(Box<Transformer>),

    // Regex:
    Capture(Regex),
    AllCaptures(Regex),
}

impl fmt::Display for Transformer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Transformer::IsNull => write!(f, "is-null"),
            Transformer::IsNotNull => write!(f, "is-not-null"),
            Transformer::Hash => write!(f, "hash"),
            Transformer::AsNumber => write!(f, "as-number"),
            Transformer::GreaterThan(num) => write!(f, "greater-than {}", num),
            Transformer::LesserThan(num) => write!(f, "lesser-than {}", num),
            Transformer::Equals(num) => write!(f, "equals {}", num),
            Transformer::Length => write!(f, "length"),
            Transformer::IsEmpty => write!(f, "is-empty"),
            Transformer::Get(key) => write!(f, "get {:?}", key),
            Transformer::GetIdx(idx) => write!(f, "get {}", idx),
            Transformer::Flatten => write!(f, "flatten"),
            Transformer::Each(transformer) => write!(f, "each({})", transformer),
            Transformer::Filter(transformer) => write!(f, "filter({})", transformer),
            Transformer::Capture(regex) => write!(f, "capture {:?}", regex.as_str()),
            Transformer::AllCaptures(regex) => write!(f, "all-captures {:?}", regex.as_str()),
        }
    }
}

impl Transformer {
    pub fn type_for(&self, input: &Type) -> Option<Type> {
        match (self, input) {
            (Transformer::IsNull, _) => Some(Type::Bool),
            (Transformer::IsNotNull, _) => Some(Type::Bool),
            (Transformer::Hash, Type::String) => Some(Type::Number),
            (Transformer::AsNumber, Type::String) => Some(Type::Number),
            (Transformer::GreaterThan(_), Type::Number) => Some(Type::Bool),
            (Transformer::LesserThan(_), Type::Number) => Some(Type::Bool),
            (Transformer::Equals(_), Type::Number) => Some(Type::Bool),
            (Transformer::Length, Type::String) => Some(Type::Number),
            (Transformer::Length, Type::Array(_)) => Some(Type::Number),
            (Transformer::Length, Type::Map(_)) => Some(Type::Number),
            (Transformer::IsEmpty, Type::String) => Some(Type::Bool),
            (Transformer::IsEmpty, Type::Array(_)) => Some(Type::Bool),
            (Transformer::IsEmpty, Type::Map(_)) => Some(Type::Bool),
            (Transformer::Get(_), Type::Map(typ)) => Some(Type::clone(&*typ)),
            (Transformer::GetIdx(_), Type::Array(typ)) => Some(Type::clone(&*typ)),
            (Transformer::Flatten, Type::Array(inner)) => {
                if let Type::Array(_) = &**inner {
                    Some(Type::clone(&*inner))
                } else {
                    None
                }
            }
            (Transformer::Each(inner), Type::Array(typ)) => {
                Some(Type::Array(Box::new(inner.type_for(typ)?)))
            }
            (Transformer::Filter(inner), Type::Array(typ)) => {
               if let Some(Type::Bool) = inner.type_for(typ) {
                    Some(Type::clone(typ))
               } else {
                   None
               }
            }
            (Transformer::Capture(_), Type::String) => Some(Type::Map(Box::new(Type::String))),
            (Transformer::AllCaptures(_), Type::String) => {
                Some(Type::Array(Box::new(Type::Map(Box::new(Type::String)))))
            }
            (_, _) => None,
        }
    }

    // #[track_caller]
    fn complain_about(&self, value: &Value) -> ! {
        panic!("type checked: {:?} {:?}", self, value)
    }

    pub fn eval(&self, input: Value) -> Value {
        match (self, input) {
            (Transformer::IsNull, Value::Null) => true.into(),
            (Transformer::IsNull, _) => false.into(),
            (Transformer::IsNotNull, Value::Null) => false.into(),
            (Transformer::IsNotNull, _) => true.into(),
            (Transformer::Hash, Value::String(string)) => crate::hash(&string).into(),
            (Transformer::AsNumber, Value::String(string)) => string
                .parse::<f64>()
                .ok()
                .map(|num| num.into())
                .unwrap_or(Value::Null),
            (&Transformer::GreaterThan(rhs), Value::Number(lhs)) => (force_f64(&lhs) > rhs).into(),
            (&Transformer::LesserThan(rhs), Value::Number(lhs)) => (force_f64(&lhs) < rhs).into(),
            (&Transformer::Equals(rhs), Value::Number(lhs)) => (force_f64(&lhs) == rhs).into(),
            (Transformer::Length, Value::Array(array)) => array.len().into(),
            (Transformer::Length, Value::String(string)) => string.len().into(),
            (Transformer::Length, Value::Object(object)) => object.len().into(),
            (Transformer::IsEmpty, Value::Array(array)) => array.is_empty().into(),
            (Transformer::IsEmpty, Value::String(string)) => string.is_empty().into(),
            (Transformer::IsEmpty, Value::Object(object)) => object.is_empty().into(),
            (Transformer::Get(ref idx), Value::Object(mut object)) => {
                object.remove(idx).unwrap_or(Value::Null)
            }
            (&Transformer::GetIdx(idx), Value::Array(array)) => {
                array.get(idx).cloned().unwrap_or(Value::Null)
            }
            (Transformer::Flatten, Value::Array(array)) => {
                let flattened = array
                    .into_iter()
                    .flat_map(|element| {
                        match element {
                            Value::Array(array) => array,
                            value => self.complain_about(&value),
                        }
                    })
                    .collect::<Vec<_>>();

                flattened.into()
            }
            (&Transformer::Each(ref inner), Value::Array(array)) => array
                .into_iter()
                .map(|value| inner.eval(value))
                .collect::<Vec<_>>()
                .into(),
            (Transformer::Filter(inner), Value::Array(array)) => array
                .into_iter()
                .filter_map(|value| {
                    match inner.eval(value.clone()) {
                        Value::Null | Value::Bool(false) => None,
                        Value::Bool(true) => Some(value),
                        value => self.complain_about(&value),
                    }
                })
                .collect::<Vec<_>>()
                .into(),
            (Transformer::Capture(regex), Value::String(string)) => regex
                .captures(&string)
                .map(|captures| capture_json(&regex, captures).into())
                .unwrap_or(Value::Null),
            (Transformer::AllCaptures(regex), Value::String(string)) => regex
                .captures_iter(&string)
                .map(|captures| capture_json(&regex, captures))
                .collect::<Vec<_>>()
                .into(),
            (_, Value::Null) => Value::Null,
            (transformer, value) => panic!("type checked: {:?} {:?}", transformer, value),
        }
    }
}
