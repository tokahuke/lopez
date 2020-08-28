use regex::{Captures, Regex};
use scraper::ElementRef;
use serde_json::{to_value, Map, Value};
use std::fmt;

use super::parse::{Aggregator, AggregatorExpression, Extractor, ExtractorExpression, Transformer};

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
    fn type_for(&self, input: &Type) -> Option<Type> {
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

impl Extractor {
    pub fn type_of(&self) -> Type {
        match self {
            Extractor::Name => Type::String,
            Extractor::Attr(_) => Type::String,
            Extractor::Html => Type::String,
            Extractor::InnerHtml => Type::String,
            Extractor::Text => Type::String,
        }
    }

    pub fn extract(&self, element_ref: ElementRef) -> Value {
        match self {
            Extractor::Name => to_value(element_ref.value().name()),
            Extractor::Attr(attr) => to_value(element_ref.value().attr(attr)),
            Extractor::Html => to_value(element_ref.html()),
            Extractor::InnerHtml => to_value(element_ref.inner_html()),
            Extractor::Text => to_value(element_ref.text().collect::<Vec<_>>().join(" ")),
        }
        .expect("can always serialize")
    }
}

impl ExtractorExpression {
    pub fn type_of(&self) -> Result<Type, crate::Error> {
        let mut typ = self.extractor.type_of();

        for transformer in &self.transformers {
            if let Some(return_type) = transformer.type_for(&typ) {
                typ = return_type;
            } else {
                return Err(crate::Error::TypeError(transformer.clone(), typ));
            }
        }

        Ok(typ)
    }

    pub fn extract(&self, element_ref: ElementRef) -> Value {
        let mut extracted = self.extractor.extract(element_ref);

        for transformer in &self.transformers {
            extracted = transformer.eval(&mut extracted);
        }

        extracted
    }
}

impl Aggregator {
    pub fn type_of(&self) -> Result<Type, crate::Error> {
        match self {
            Aggregator::Count => Ok(Type::Number),
            Aggregator::CountNotNull(extractor_expr) => {
                extractor_expr.type_of()?;
                Ok(Type::Number)
            }
            Aggregator::First(extractor_expr) => extractor_expr.type_of(),
            Aggregator::Collect(extractor_expr) => {
                Ok(Type::Array(Box::new(extractor_expr.type_of()?)))
            }
        }
    }
}

impl AggregatorExpression {
    pub fn type_of(&self) -> Result<Type, crate::Error> {
        let mut typ = self.aggregator.type_of()?;

        for transformer in &self.transformers {
            if let Some(return_type) = transformer.type_for(&typ) {
                typ = return_type;
            } else {
                return Err(crate::Error::TypeError(transformer.clone(), typ));
            }
        }

        Ok(typ)
    }
}

pub enum AggregatorState<'a> {
    Count(usize),
    CountNotNull(&'a ExtractorExpression, usize),
    First(&'a ExtractorExpression, Option<Value>),
    Collect(&'a ExtractorExpression, Vec<Value>),
}

impl<'a> AggregatorState<'a> {
    pub fn new(aggregator: &Aggregator) -> AggregatorState {
        match aggregator {
            Aggregator::Count => AggregatorState::Count(0),
            Aggregator::CountNotNull(extractor_expr) => {
                AggregatorState::CountNotNull(extractor_expr, 0)
            }
            Aggregator::First(extractor_expr) => AggregatorState::First(extractor_expr, None),
            Aggregator::Collect(extractor_expr) => AggregatorState::Collect(extractor_expr, vec![]),
        }
    }

    pub fn aggregate(&mut self, element_ref: ElementRef) {
        match self {
            AggregatorState::Count(count) => *count += 1,
            AggregatorState::CountNotNull(extractor, count) => {
                if !extractor.extract(element_ref).is_null() {
                    *count += 1;
                }
            }
            AggregatorState::First(extractor, maybe_value) => {
                if maybe_value.is_none() {
                    *maybe_value = Some(extractor.extract(element_ref))
                }
            }
            AggregatorState::Collect(extractor, values) => {
                values.push(extractor.extract(element_ref));
            }
        }
    }

    pub fn finalize(self) -> Value {
        match self {
            AggregatorState::Count(count) => count.into(),
            AggregatorState::CountNotNull(_, count) => count.into(),
            AggregatorState::First(_, value) => value.unwrap_or_default(),
            AggregatorState::Collect(_, collected) => collected.into(),
        }
    }
}

pub struct AggregatorExpressionState<'a> {
    state: AggregatorState<'a>,
    transformers: &'a [Transformer],
}

impl<'a> AggregatorExpressionState<'a> {
    pub fn new(aggregator_expr: &AggregatorExpression) -> AggregatorExpressionState {
        AggregatorExpressionState {
            state: AggregatorState::new(&aggregator_expr.aggregator),
            transformers: &aggregator_expr.transformers,
        }
    }

    pub fn aggregate(&mut self, element_ref: ElementRef) {
        self.state.aggregate(element_ref)
    }

    pub fn finalize(self) -> Value {
        let mut finalized = self.state.finalize();

        for transformer in self.transformers {
            finalized = transformer.eval(&mut finalized);
        }

        finalized
    }
}
