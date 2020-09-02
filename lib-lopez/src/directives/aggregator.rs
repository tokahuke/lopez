use scraper::ElementRef;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashSet};
use std::fmt;

use super::value_ext::{force_f64, HashableJson};

use super::extractor::ExplodingExtractorExpression;
use super::transformer::{TransformerExpression, Type};

#[derive(Debug, PartialEq)]
pub enum Aggregator {
    Count,
    CountNotNull(ExplodingExtractorExpression),
    First(ExplodingExtractorExpression),
    Collect(ExplodingExtractorExpression),
    Distinct(ExplodingExtractorExpression),
    Sum(ExplodingExtractorExpression),
    Group(ExplodingExtractorExpression, Box<AggregatorExpression>),
}

impl fmt::Display for Aggregator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Aggregator::Count => write!(f, "count"),
            Aggregator::CountNotNull(extractor_expr) => write!(f, "count({})", extractor_expr),
            Aggregator::First(extractor_expr) => write!(f, "first({})", extractor_expr),
            Aggregator::Collect(extractor_expr) => write!(f, "collect({})", extractor_expr),
            Aggregator::Distinct(extractor_expr) => write!(f, "distinct({})", extractor_expr),
            Aggregator::Sum(extractor_expr) => write!(f, "sum({})", extractor_expr),
            Aggregator::Group(extractor_expr, aggregator_expr) => {
                write!(f, "group({}, {})", extractor_expr, aggregator_expr)
            }
        }
    }
}

impl Aggregator {
    fn type_error<T>(&self, input: &Type) -> Result<T, crate::Error> {
        Err(crate::Error::TypeError(self.to_string(), input.clone()))
    }

    pub fn type_of(&self) -> Result<Type, crate::Error> {
        match self {
            Aggregator::Count => Ok(Type::Number),
            Aggregator::CountNotNull(extractor_expr) => {
                let typ = extractor_expr.type_of()?;
                if let Type::Bool = typ {
                    Ok(Type::Number)
                } else {
                    self.type_error(&typ)
                }
            }
            Aggregator::First(extractor_expr) => extractor_expr.type_of(),
            Aggregator::Collect(extractor_expr) => {
                Ok(Type::Array(Box::new(extractor_expr.type_of()?)))
            }
            Aggregator::Distinct(extractor_expr) => {
                Ok(Type::Array(Box::new(extractor_expr.type_of()?)))
            }
            Aggregator::Sum(extractor_expr) => {
                let typ = extractor_expr.type_of()?;
                if let Type::Number = extractor_expr.type_of()? {
                    Ok(Type::Number)
                } else {
                    self.type_error(&typ)
                }
            }
            Aggregator::Group(extractor_expr, aggregator_expr) => {
                let extract_type = extractor_expr.type_of()?;
                let aggregator_type = aggregator_expr.type_of()?;

                if let Type::String = extract_type {
                    Ok(Type::Map(Box::new(aggregator_type)))
                } else {
                    self.type_error(&extract_type)
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct AggregatorExpression {
    pub aggregator: Aggregator,
    pub transformer_expression: TransformerExpression,
}

impl fmt::Display for AggregatorExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.transformer_expression.is_empty() {
            write!(f, "{}", self.aggregator)
        } else {
            write!(f, "{} ", self.aggregator)?;
            write!(f, "{}", self.transformer_expression)
        }
    }
}

impl AggregatorExpression {
    pub fn type_of(&self) -> Result<Type, crate::Error> {
        self.transformer_expression
            .type_for(&self.aggregator.type_of()?)
    }
}

#[derive(Debug)]
pub(crate) enum AggregatorState<'a> {
    Count(usize),
    CountNotNull(&'a ExplodingExtractorExpression, usize),
    First(&'a ExplodingExtractorExpression, Option<Value>),
    Collect(&'a ExplodingExtractorExpression, Vec<Value>),
    Distinct(&'a ExplodingExtractorExpression, HashSet<HashableJson>),
    Sum(&'a ExplodingExtractorExpression, f64),
    Group(
        &'a ExplodingExtractorExpression,
        &'a AggregatorExpression,
        BTreeMap<String, AggregatorExpressionState<'a>>,
    ),
}

impl<'a> AggregatorState<'a> {
    #[track_caller]
    fn complain_about(&self, value: &Value) -> ! {
        panic!("type checked: {:#?} at {}", self, value)
    }

    pub fn new(aggregator: &Aggregator) -> AggregatorState {
        match aggregator {
            Aggregator::Count => AggregatorState::Count(0),
            Aggregator::CountNotNull(extractor_expr) => {
                AggregatorState::CountNotNull(extractor_expr, 0)
            }
            Aggregator::First(extractor_expr) => AggregatorState::First(extractor_expr, None),
            Aggregator::Collect(extractor_expr) => AggregatorState::Collect(extractor_expr, vec![]),
            Aggregator::Distinct(extractor_expr) => {
                AggregatorState::Distinct(extractor_expr, HashSet::new())
            }
            Aggregator::Sum(extractor_expr) => AggregatorState::Sum(extractor_expr, 0.),
            Aggregator::Group(extractor_expr, aggregator_expr) => {
                AggregatorState::Group(extractor_expr, aggregator_expr, BTreeMap::new())
            }
        }
    }

    #[inline]
    pub fn aggregate(&mut self, element_ref: ElementRef) {
        match self {
            AggregatorState::Count(count) => *count += 1,
            AggregatorState::CountNotNull(extractor, count) => {
                for value in extractor.extract(element_ref) {
                    match value {
                        Value::Bool(true) => *count += 1,
                        Value::Bool(false) | Value::Null => {}
                        value => self.complain_about(&value),
                    }
                }
            }
            AggregatorState::First(extractor, maybe_value) => {
                if maybe_value.is_none() {
                    for value in extractor.extract(element_ref) {
                        if !value.is_null() {
                            *maybe_value = Some(value);
                            break;
                        }
                    }
                }
            }
            AggregatorState::Collect(extractor, values) => {
                values.extend(extractor.extract(element_ref));
            }
            AggregatorState::Distinct(extractor, values) => {
                values.extend(extractor.extract(element_ref).into_iter().map(HashableJson));
            }
            AggregatorState::Sum(extractor, sum) => {
                for value in extractor.extract(element_ref) {
                    if let Value::Number(num) = value {
                        *sum += force_f64(&num);
                    } else if !value.is_null() {
                        self.complain_about(&value)
                    }
                }
            }
            AggregatorState::Group(extractor_expr, aggregator_expr, groups) => {
                for key in extractor_expr.extract(element_ref) {
                    if let Value::String(key) = key {
                        groups
                            .entry(key)
                            .or_insert_with(|| AggregatorExpressionState::new(aggregator_expr))
                            .aggregate(element_ref)
                    } else if !key.is_null() {
                        self.complain_about(&key)
                    }
                }
            }
        }
    }

    pub fn finalize(self) -> Value {
        match self {
            AggregatorState::Count(count) => count.into(),
            AggregatorState::CountNotNull(_, count) => count.into(),
            AggregatorState::First(_, value) => value.unwrap_or_default(),
            AggregatorState::Collect(_, collected) => collected.into(),
            AggregatorState::Distinct(_, distinct) => distinct
                .into_iter()
                .map(|hashable_json| hashable_json.0)
                .collect::<Vec<_>>()
                .into(),
            AggregatorState::Sum(_, sum) => sum.into(),
            AggregatorState::Group(_, _, groups) => groups
                .into_iter()
                .map(|(key, state)| (key, state.finalize()))
                .collect::<Map<_, _>>()
                .into(),
        }
    }
}

#[derive(Debug)]
pub struct AggregatorExpressionState<'a> {
    state: AggregatorState<'a>,
    transformer_expression: &'a TransformerExpression,
}

impl<'a> AggregatorExpressionState<'a> {
    pub fn new(aggregator_expr: &AggregatorExpression) -> AggregatorExpressionState {
        AggregatorExpressionState {
            state: AggregatorState::new(&aggregator_expr.aggregator),
            transformer_expression: &aggregator_expr.transformer_expression,
        }
    }

    pub fn aggregate(&mut self, element_ref: ElementRef) {
        self.state.aggregate(element_ref)
    }

    pub fn finalize(self) -> Value {
        self.transformer_expression.eval(self.state.finalize())
    }
}
