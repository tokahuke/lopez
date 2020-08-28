use scraper::ElementRef;
use serde_json::{Value};

use super::transformer::{Type, Transformer};
use super::extractor::ExtractorExpression;

#[derive(Debug, Clone)]
pub enum Aggregator {
    Count,
    CountNotNull(ExtractorExpression),
    First(ExtractorExpression),
    Collect(ExtractorExpression),
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

#[derive(Debug, Clone)]
pub struct AggregatorExpression {
    pub aggregator: Aggregator,
    pub transformers: Vec<Transformer>,
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
