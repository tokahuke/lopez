use regex::{Captures, Regex};
use serde_derive::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{cmp, fmt};

use super::value_ext::force_f64;
use super::{Error, Type};

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

/// Prettifies text, removing unnecessary whitespace.
fn pretty(i: &str) -> String {
    let stream = i.split('\n').map(|paragraph| {
        paragraph
            .split_whitespace()
            .map(|word| word.trim())
            .filter(|word| !word.is_empty())
    });
    let mut pretty = String::with_capacity(i.len());
    let mut p_sep = None;

    for chunks in stream {
        if let Some(p_sep) = p_sep {
            pretty += p_sep;
        }

        let mut sep = None;

        for chunk in chunks {
            if let Some(sep) = sep {
                pretty += sep;
            }

            pretty += chunk;

            sep = Some(" ");
        }

        p_sep = if sep.is_some() { Some("\n") } else { None };
    }

    // Post-process:
    if !pretty.is_empty() && !pretty.ends_with('\n') {
        pretty += "\n";
    }

    pretty
}

#[test]
fn pretty_test() {
    let ugly = "\n\n\n\n\t    \r\r\n\n ";
    assert_eq!("", pretty(ugly));

    let ugly = "\n\na\n\n\t    \r\rb\n\n ";
    assert_eq!("a\nb\n", pretty(ugly));

    let ugly = "\n\n\na\n\t    \r\r\n\n ";
    assert_eq!("a\n", pretty(ugly));

    let ugly = "\n\n\na\n\t    \r\r\n\n c";
    assert_eq!("a\nc\n", pretty(ugly));
}

/// Need this to shoehorn regex equality. Note: this is utterly broken in a
/// context wider than unittesting.
#[derive(Debug, Deserialize, Serialize)]
pub struct ComparableRegex(#[serde(with = "serde_regex")] pub Regex);

impl PartialEq for ComparableRegex {
    fn eq(&self, other: &ComparableRegex) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

fn cmp_json(this: &Value, other: &Value) -> cmp::Ordering {
    match (this, other) {
        (Value::Null, Value::Null) => cmp::Ordering::Equal,
        (Value::Null, _) => cmp::Ordering::Less,
        (_, Value::Null) => cmp::Ordering::Greater,
        (Value::Bool(this), Value::Bool(other)) => this.cmp(&other),
        (Value::Number(this), Value::Number(other)) => force_f64(this)
            .partial_cmp(&force_f64(other))
            .expect("json Number cannot be NaN"),
        (Value::String(this), Value::String(other)) => this.cmp(&other),
        (Value::Array(this), Value::Array(other)) => {
            for (this, other) in this.iter().zip(other) {
                match cmp_json(this, other) {
                    cmp::Ordering::Equal => {}
                    outcome => return outcome,
                }
            }

            cmp::Ordering::Equal
        }
        (Value::Object(_), Value::Object(_)) => panic!("comparing objects is not defined (yet)"),
        _ => panic!("comparing different types: {} and {}", this, other),
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub enum Transformer {
    // General purpose:
    IsNull,
    IsNotNull,
    Hash,
    Not,

    // Numeric:
    AsNumber,
    GreaterThan(f64),
    LesserThan(f64),
    GreaterOrEqual(f64), // missing docs!
    LesserOrEqual(f64),  // missing docs!
    Between(f64, f64),   // missing docs!
    Equals(f64),
    In(Box<[f64]>), // missing docs!

    // Collections:
    Length,
    IsEmpty,
    Get(Box<str>),
    GetIdx(usize),
    Flatten,
    Each(TransformerExpression),
    Filter(TransformerExpression),
    Any(TransformerExpression),    // missing docs!
    All(TransformerExpression),    // missing docs!
    Sort,                          // missing docs!
    SortBy(TransformerExpression), // missing docs!

    // String manipulation
    AsString,
    Pretty,
    EqualsString(Box<str>),     // missing docs!
    InStrings(Box<[Box<str>]>), // missing docs!

    // Regex:
    Capture(ComparableRegex),
    AllCaptures(ComparableRegex),
    Matches(ComparableRegex),
    Replace(ComparableRegex, Box<str>),
}

impl fmt::Display for Transformer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Transformer::IsNull => write!(f, "is-null"),
            Transformer::IsNotNull => write!(f, "is-not-null"),
            Transformer::Hash => write!(f, "hash"),
            Transformer::Not => write!(f, "not"),
            Transformer::AsNumber => write!(f, "as-number"),
            Transformer::GreaterThan(num) => write!(f, "greater-than {}", num),
            Transformer::LesserThan(num) => write!(f, "lesser-than {}", num),
            Transformer::GreaterOrEqual(num) => write!(f, "greater-or-equal {}", num),
            Transformer::LesserOrEqual(num) => write!(f, "lesser-or-equal {}", num),
            Transformer::Between(low, high) => write!(f, "between {} and {}", low, high),
            Transformer::Equals(num) => write!(f, "equals {}", num),
            Transformer::In(nums) => write!(
                f,
                "in [{}]",
                nums.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Transformer::Length => write!(f, "length"),
            Transformer::IsEmpty => write!(f, "is-empty"),
            Transformer::Get(key) => write!(f, "get {:?}", key),
            Transformer::GetIdx(idx) => write!(f, "get {}", idx),
            Transformer::Flatten => write!(f, "flatten"),
            Transformer::Each(transformer) => write!(f, "each({})", transformer),
            Transformer::Filter(transformer) => write!(f, "filter({})", transformer),
            Transformer::Any(transformer) => write!(f, "any({})", transformer),
            Transformer::All(transformer) => write!(f, "all({})", transformer),
            Transformer::Sort => write!(f, "sort"),
            Transformer::SortBy(transformer) => write!(f, "sort-by({})", transformer),
            Transformer::Capture(ComparableRegex(regex)) => {
                write!(f, "capture {:?}", regex.as_str())
            }
            Transformer::AsString => write!(f, "as-string"),
            Transformer::Pretty => write!(f, "pretty"),
            Transformer::EqualsString(string) => write!(f, "equals {:?}", string),
            Transformer::InStrings(strings) => write!(f, "in {:?}", strings),
            Transformer::AllCaptures(ComparableRegex(regex)) => {
                write!(f, "all-captures {:?}", regex.as_str())
            }
            Transformer::Matches(ComparableRegex(regex)) => {
                write!(f, "matches {:?}", regex.as_str())
            }
            Transformer::Replace(ComparableRegex(regex), replacer) => {
                write!(f, "replace {:?} with {:?}", regex, replacer)
            }
        }
    }
}

impl Transformer {
    fn type_error<T>(&self, input: &Type) -> Result<T, Error> {
        Err(Error::TypeError(self.to_string(), input.clone()))
    }

    pub fn type_for(&self, input: &Type) -> Result<Type, Error> {
        match (self, input) {
            (Transformer::IsNull, _) => Ok(Type::Bool),
            (Transformer::IsNotNull, _) => Ok(Type::Bool),
            (Transformer::Hash, Type::String) => Ok(Type::Number),
            (Transformer::Not, Type::Bool) => Ok(Type::Bool),
            (Transformer::AsNumber, Type::String) => Ok(Type::Number),
            (Transformer::GreaterThan(_), Type::Number) => Ok(Type::Bool),
            (Transformer::LesserThan(_), Type::Number) => Ok(Type::Bool),
            (Transformer::GreaterOrEqual(_), Type::Number) => Ok(Type::Bool),
            (Transformer::LesserOrEqual(_), Type::Number) => Ok(Type::Bool),
            (Transformer::Between(_, _), Type::Number) => Ok(Type::Bool),
            (Transformer::Equals(_), Type::Number) => Ok(Type::Bool),
            (Transformer::In(_), Type::Number) => Ok(Type::Bool),
            (Transformer::Length, Type::String) => Ok(Type::Number),
            (Transformer::Length, Type::Array(_)) => Ok(Type::Number),
            (Transformer::Length, Type::Map(_)) => Ok(Type::Number),
            (Transformer::IsEmpty, Type::String) => Ok(Type::Bool),
            (Transformer::IsEmpty, Type::Array(_)) => Ok(Type::Bool),
            (Transformer::IsEmpty, Type::Map(_)) => Ok(Type::Bool),
            (Transformer::Get(_), Type::Map(typ)) => Ok(Type::clone(&*typ)),
            (Transformer::GetIdx(_), Type::Array(typ)) => Ok(Type::clone(&*typ)),
            (Transformer::Flatten, Type::Array(inner)) => {
                if let Type::Array(_) = &**inner {
                    Ok(Type::clone(&*inner))
                } else {
                    self.type_error(input)
                }
            }
            (Transformer::Each(inner), Type::Array(typ)) => {
                Ok(Type::Array(Box::new(inner.type_for(typ)?)))
            }
            (Transformer::Each(inner), Type::Map(typ)) => {
                Ok(Type::Map(Box::new(inner.type_for(typ)?)))
            }
            (Transformer::Filter(inner), Type::Array(typ)) => {
                let inner_typ = inner.type_for(typ)?;
                if let Type::Bool = &inner_typ {
                    Ok(Type::Array(typ.clone()))
                } else {
                    inner.expected(&Type::Bool, &inner_typ)
                }
            }
            (Transformer::Filter(inner), Type::Map(typ)) => {
                let inner_typ = inner.type_for(typ)?;
                if let Type::Bool = &inner_typ {
                    Ok(Type::Map(typ.clone()))
                } else {
                    inner.expected(&Type::Bool, &inner_typ)
                }
            }
            (Transformer::Any(predicate), Type::Array(typ)) => {
                let predicate_typ = predicate.type_for(typ)?;
                if let Type::Bool = predicate_typ {
                    Ok(Type::Bool)
                } else {
                    predicate.expected(&Type::Bool, &predicate_typ)
                }
            }
            (Transformer::All(predicate), Type::Array(typ)) => {
                let predicate_typ = predicate.type_for(typ)?;
                if let Type::Bool = predicate_typ {
                    Ok(Type::Bool)
                } else {
                    predicate.expected(&Type::Bool, &predicate_typ)
                }
            }
            (Transformer::Sort, Type::Array(typ)) if !typ.is_map() => Ok(Type::Array(typ.clone())),
            (Transformer::SortBy(key), Type::Array(typ)) => {
                let key_typ = key.type_for(typ)?;
                if !key_typ.is_map() {
                    Ok(Type::Array(typ.clone()))
                } else {
                    key.not_expected(&key_typ)
                }
            }
            (Transformer::AsString, Type::Number) => Ok(Type::String),
            (Transformer::AsString, Type::Bool) => Ok(Type::String),
            (Transformer::AsString, Type::String) => Ok(Type::String),
            (Transformer::Pretty, Type::String) => Ok(Type::String),
            (Transformer::EqualsString(_), Type::String) => Ok(Type::Bool),
            (Transformer::InStrings(_), Type::String) => Ok(Type::Bool),
            (Transformer::Capture(_), Type::String) => Ok(Type::Map(Box::new(Type::String))),
            (Transformer::AllCaptures(_), Type::String) => {
                Ok(Type::Array(Box::new(Type::Map(Box::new(Type::String)))))
            }
            (Transformer::Matches(_), Type::String) => Ok(Type::Bool),
            (Transformer::Replace(_, _), Type::String) => Ok(Type::String),
            (_, _) => self.type_error(input),
        }
    }

    #[track_caller]
    fn complain_about(&self, value: &Value) -> ! {
        panic!("type checked: {:?} {:?}", self, value)
    }

    #[inline(always)]
    pub fn eval(&self, input: Value) -> Value {
        match (self, input) {
            (Transformer::IsNull, Value::Null) => true.into(),
            (Transformer::IsNull, _) => false.into(),
            (Transformer::IsNotNull, Value::Null) => false.into(),
            (Transformer::IsNotNull, _) => true.into(),
            (Transformer::Not, Value::Bool(b)) => (!b).into(),
            (Transformer::Hash, Value::String(string)) => crate::hash(&string).into(),
            (Transformer::AsNumber, Value::String(string)) => string
                .parse::<f64>()
                .ok()
                .map(|num| num.into())
                .unwrap_or(Value::Null),
            (&Transformer::GreaterThan(rhs), Value::Number(lhs)) => (force_f64(&lhs) > rhs).into(),
            (&Transformer::LesserThan(rhs), Value::Number(lhs)) => (force_f64(&lhs) < rhs).into(),
            (&Transformer::GreaterOrEqual(rhs), Value::Number(lhs)) => {
                (force_f64(&lhs) > rhs).into()
            }
            (&Transformer::LesserOrEqual(rhs), Value::Number(lhs)) => {
                (force_f64(&lhs) <= rhs).into()
            }
            (&Transformer::Between(low, high), Value::Number(lhs)) => {
                (force_f64(&lhs) >= low && force_f64(&lhs) <= high).into()
            }
            (&Transformer::Equals(rhs), Value::Number(lhs)) => {
                ((force_f64(&lhs) - rhs).abs() < std::f64::EPSILON).into()
            }
            (Transformer::In(nums), Value::Number(lhs)) => nums
                .iter()
                .any(|rhs| ((force_f64(&lhs) - rhs).abs() < std::f64::EPSILON))
                .into(),
            (Transformer::Length, Value::Array(array)) => array.len().into(),
            (Transformer::Length, Value::String(string)) => string.len().into(),
            (Transformer::Length, Value::Object(object)) => object.len().into(),
            (Transformer::IsEmpty, Value::Array(array)) => array.is_empty().into(),
            (Transformer::IsEmpty, Value::String(string)) => string.is_empty().into(),
            (Transformer::IsEmpty, Value::Object(object)) => object.is_empty().into(),
            (Transformer::Get(ref idx), Value::Object(mut object)) => {
                object.remove(idx.as_ref()).unwrap_or(Value::Null)
            }
            (&Transformer::GetIdx(idx), Value::Array(array)) => {
                array.get(idx).cloned().unwrap_or(Value::Null)
            }
            (Transformer::Flatten, Value::Array(array)) => {
                let flattened = array
                    .into_iter()
                    .filter_map(|element| match element {
                        Value::Array(array) => Some(array),
                        Value::Null => None,
                        value => self.complain_about(&value),
                    })
                    .flatten()
                    .collect::<Vec<_>>();

                flattened.into()
            }
            (&Transformer::Each(ref inner), Value::Array(array)) => array
                .into_iter()
                .map(|value| inner.eval(value))
                .collect::<Vec<_>>()
                .into(),
            (&Transformer::Each(ref inner), Value::Object(map)) => map
                .into_iter()
                .map(|(key, value)| (key, inner.eval(value)))
                .collect::<Map<String, Value>>()
                .into(),
            (Transformer::Filter(inner), Value::Array(array)) => array
                .into_iter()
                .filter_map(|value| match inner.eval(value.clone()) {
                    Value::Null | Value::Bool(false) => None,
                    Value::Bool(true) => Some(value),
                    value => self.complain_about(&value),
                })
                .collect::<Vec<_>>()
                .into(),
            (Transformer::Filter(inner), Value::Object(map)) => map
                .into_iter()
                .filter_map(|(key, value)| match inner.eval(value.clone()) {
                    Value::Null | Value::Bool(false) => None,
                    Value::Bool(true) => Some((key, value)),
                    value => self.complain_about(&value),
                })
                .collect::<Map<String, Value>>()
                .into(),
            (Transformer::Any(predicate), Value::Array(array)) => array
                .into_iter()
                .any(|value| match predicate.eval(value) {
                    Value::Null | Value::Bool(false) => false,
                    Value::Bool(true) => true,
                    value => self.complain_about(&value),
                })
                .into(),
            (Transformer::All(predicate), Value::Array(array)) => array
                .into_iter()
                .all(|value| match predicate.eval(value) {
                    Value::Null | Value::Bool(false) => false,
                    Value::Bool(true) => true,
                    value => self.complain_about(&value),
                })
                .into(),
            (Transformer::Sort, Value::Array(array)) => {
                let mut array = array.clone();
                array.sort_unstable_by(cmp_json);
                array.into()
            }
            (Transformer::SortBy(key), Value::Array(array)) => {
                let mut array = array.clone();
                array.sort_unstable_by(|a, b| cmp_json(&key.eval(a.clone()), &key.eval(b.clone())));
                array.into()
            }
            (Transformer::AsString, Value::Number(num)) => num.to_string().into(),
            (Transformer::AsString, Value::Bool(b)) => b.to_string().into(),
            (Transformer::AsString, Value::String(string)) => Value::String(string),
            (Transformer::Pretty, Value::String(string)) => pretty(&string).into(),
            (Transformer::EqualsString(this), Value::String(other)) => {
                (this.as_ref() == other.as_str()).into()
            }
            (Transformer::InStrings(strings), Value::String(other)) => strings
                .iter()
                .any(|string| string.as_ref() == other.as_str())
                .into(),
            (Transformer::Capture(ComparableRegex(regex)), Value::String(string)) => regex
                .captures(&string)
                .map(|captures| capture_json(&regex, captures).into())
                .unwrap_or(Value::Null),
            (Transformer::AllCaptures(ComparableRegex(regex)), Value::String(string)) => regex
                .captures_iter(&string)
                .map(|captures| capture_json(&regex, captures))
                .collect::<Vec<_>>()
                .into(),
            (Transformer::Matches(ComparableRegex(regex)), Value::String(string)) => {
                regex.is_match(&string).into()
            }
            (Transformer::Replace(ComparableRegex(regex), replacer), Value::String(string)) => {
                regex
                    .replace_all(&string, replacer.as_ref())
                    .into_owned()
                    .into()
            }
            (_, Value::Null) => Value::Null,
            (transformer, value) => panic!("type checked: {:?} {:?}", transformer, value),
        }
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct TransformerExpression {
    pub transformers: Box<[Transformer]>,
}

impl fmt::Display for TransformerExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iter = self.transformers.iter();

        if let Some(transformer) = iter.next() {
            write!(f, "{}", transformer)?;
        }

        for transformer in iter {
            write!(f, " {}", transformer)?;
        }

        Ok(())
    }
}

impl TransformerExpression {
    pub fn is_empty(&self) -> bool {
        self.transformers.is_empty()
    }

    fn expected<T>(&self, expected: &Type, got: &Type) -> Result<T, Error> {
        Err(Error::ExpectedType {
            thing: self.to_string(),
            expected: expected.clone(),
            got: got.clone(),
        })
    }

    fn not_expected<T>(&self, not_expected: &Type) -> Result<T, Error> {
        Err(Error::NotExpectedType {
            thing: self.to_string(),
            not_expected: not_expected.clone(),
        })
    }

    pub fn type_for(&self, input: &Type) -> Result<Type, Error> {
        let mut typ = input.clone();

        for transformer in &*self.transformers {
            typ = transformer.type_for(&typ)?;
        }

        Ok(typ)
    }

    pub fn eval(&self, mut value: Value) -> Value {
        for transformer in &*self.transformers {
            value = transformer.eval(value);
        }

        value
    }
}
