use serde_json::{Map, Value};
use smallvec::SmallVec;
use std::collections::{BTreeMap, HashSet};
use std::fmt;

use super::value_ext::{force_f64, HashableJson};

use super::extractor::ExplodingExtractorExpression;
use super::transformer::TransformerExpression;
use super::{Error, Extractable, Type, Typed};

#[derive(Debug, PartialEq)]
pub enum Aggregator<E: Typed> {
    Count,
    CountNotNull(ExplodingExtractorExpression<E>),
    First(ExplodingExtractorExpression<E>),
    Collect(ExplodingExtractorExpression<E>),
    Distinct(ExplodingExtractorExpression<E>),
    Sum(ExplodingExtractorExpression<E>),
    Group(
        ExplodingExtractorExpression<E>,
        Box<AggregatorExpression<E>>,
    ),
}

impl<E: Typed> fmt::Display for Aggregator<E> {
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

impl<E: Typed> Aggregator<E> {
    fn type_error<U>(&self, input: &Type) -> Result<U, Error> {
        Err(Error::TypeError(self.to_string(), input.clone()))
    }

    pub fn type_of(&self) -> Result<Type, Error> {
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
pub struct AggregatorExpression<E: Typed> {
    pub aggregator: Aggregator<E>,
    pub transformer_expression: TransformerExpression,
}

impl<E: Typed> fmt::Display for AggregatorExpression<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.transformer_expression.is_empty() {
            write!(f, "{}", self.aggregator)
        } else {
            write!(f, "{} ", self.aggregator)?;
            write!(f, "{}", self.transformer_expression)
        }
    }
}

impl<E: Typed> AggregatorExpression<E> {
    pub fn type_of(&self) -> Result<Type, Error> {
        self.transformer_expression
            .type_for(&self.aggregator.type_of()?)
    }
}

#[derive(Debug)]
pub(crate) enum AggregatorState<'a, E: Typed> {
    Count(usize),
    CountNotNull(&'a ExplodingExtractorExpression<E>, usize),
    First(&'a ExplodingExtractorExpression<E>, Option<Value>),
    Collect(&'a ExplodingExtractorExpression<E>, Vec<Value>),
    Distinct(&'a ExplodingExtractorExpression<E>, HashSet<HashableJson>),
    Sum(&'a ExplodingExtractorExpression<E>, f64),
    Group(
        &'a ExplodingExtractorExpression<E>,
        &'a AggregatorExpression<E>,
        BTreeMap<String, AggregatorExpressionState<'a, E>>,
    ),
}

impl<'a, E: Typed> AggregatorState<'a, E> {
    #[track_caller]
    fn complain_about(&self, value: &Value) -> ! {
        panic!("type checked: {:#?} at {}", self, value)
    }

    pub fn new(aggregator: &'a Aggregator<E>) -> Self {
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
    pub fn aggregate<T>(&mut self, operand: T)
    where
        T: Extractable<ExplodingExtractorExpression<E>, Output = SmallVec<[Value; 1]>>,
    {
        match self {
            AggregatorState::Count(count) => *count += 1,
            AggregatorState::CountNotNull(extractor, count) => {
                for value in operand.extract_with(&extractor) {
                    match value {
                        Value::Bool(true) => *count += 1,
                        Value::Bool(false) | Value::Null => {}
                        value => self.complain_about(&value),
                    }
                }
            }
            AggregatorState::First(extractor, maybe_value) => {
                if maybe_value.is_none() {
                    for value in operand.extract_with(&extractor) {
                        if !value.is_null() {
                            *maybe_value = Some(value);
                            break;
                        }
                    }
                }
            }
            AggregatorState::Collect(extractor, values) => {
                values.extend(operand.extract_with(&extractor));
            }
            AggregatorState::Distinct(extractor, values) => {
                values.extend(
                    operand
                        .extract_with(&extractor)
                        .into_iter()
                        .map(HashableJson),
                );
            }
            AggregatorState::Sum(extractor, sum) => {
                for value in operand.extract_with(&extractor) {
                    if let Value::Number(num) = value {
                        *sum += force_f64(&num);
                    } else if !value.is_null() {
                        self.complain_about(&value)
                    }
                }
            }
            AggregatorState::Group(extractor_expr, aggregator_expr, groups) => {
                for key in operand.extract_with(&extractor_expr) {
                    if let Value::String(key) = key {
                        groups
                            .entry(key)
                            .or_insert_with(|| AggregatorExpressionState::<E>::new(aggregator_expr))
                            .aggregate(operand)
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
pub struct AggregatorExpressionState<'a, E: Typed> {
    state: AggregatorState<'a, E>,
    transformer_expression: &'a TransformerExpression,
}

impl<'a, E: Typed> AggregatorExpressionState<'a, E> {
    pub fn new(aggregator_expr: &'a AggregatorExpression<E>) -> AggregatorExpressionState<E> {
        AggregatorExpressionState {
            state: AggregatorState::new(&aggregator_expr.aggregator),
            transformer_expression: &aggregator_expr.transformer_expression,
        }
    }

    pub fn aggregate<T>(&mut self, operand: T)
    where
        T: Extractable<ExplodingExtractorExpression<E>, Output = SmallVec<[Value; 1]>>,
    {
        self.state.aggregate(operand)
    }

    pub fn finalize(self) -> Value {
        self.transformer_expression.eval(self.state.finalize())
    }
}
