use regex::{Captures, Regex};
use serde_json::{Map, Value};
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
    Length,
    IsNull,
    IsNotNull,
    Hash,
    Get(String),
    GetIdx(usize),
    Flatten,
    Each(Box<Transformer>),
    Capture(Regex),
    AllCaptures(Regex),
}

impl fmt::Display for Transformer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Transformer::Length => write!(f, "length"),
            Transformer::IsNull => write!(f, "is-null"),
            Transformer::IsNotNull => write!(f, "is-not-null"),
            Transformer::Hash => write!(f, "hash"),
            Transformer::Get(key) => write!(f, "get {:?}", key),
            Transformer::GetIdx(idx) => write!(f, "get {}", idx),
            Transformer::Flatten => write!(f, "flatten"),
            Transformer::Each(transformer) => write!(f, "each({})", transformer),
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
            (Transformer::Length, Type::Array(_)) => Some(Type::Number),
            (Transformer::Length, Type::String) => Some(Type::Number),
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
            (Transformer::Capture(_), Type::String) => Some(Type::Map(Box::new(Type::String))),
            (Transformer::AllCaptures(_), Type::String) => {
                Some(Type::Array(Box::new(Type::Map(Box::new(Type::String)))))
            }
            (_, _) => None,
        }
    }

    pub fn eval(&self, input: &mut Value) -> Value {
        match (self, input) {
            (Transformer::IsNull, Value::Null) => true.into(),
            (Transformer::IsNull, _) => false.into(),
            (Transformer::IsNotNull, Value::Null) => false.into(),
            (Transformer::IsNotNull, _) => true.into(),
            (Transformer::Hash, Value::String(string)) => crate::hash(&*string).into(),
            (Transformer::Length, Value::Array(array)) => array.len().into(),
            (Transformer::Length, Value::String(string)) => string.len().into(),
            (Transformer::Length, Value::Object(object)) => object.len().into(),
            (Transformer::Get(idx), Value::Object(object)) => {
                object.remove(idx).unwrap_or(Value::Null)
            }
            (Transformer::GetIdx(idx), Value::Array(array)) => {
                array.get(*idx).cloned().unwrap_or(Value::Null)
            }
            (Transformer::Flatten, Value::Array(array)) => {
                let flattened = array
                    .iter_mut()
                    .flat_map(|element| {
                        if let Value::Array(array) = element.take() {
                            array
                        } else {
                            panic!("type checked: {:?} {:?}", Transformer::Flatten, element,);
                        }
                    })
                    .collect::<Vec<_>>();

                flattened.into()
            }
            (Transformer::Each(inner), Value::Array(array)) => array
                .iter_mut()
                .map(|value| inner.eval(value))
                .collect::<Vec<_>>()
                .into(),
            (Transformer::Capture(regex), Value::String(string)) => regex
                .captures(&string)
                .map(|captures| capture_json(&regex, captures))
                .unwrap_or_default()
                .into(),
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
