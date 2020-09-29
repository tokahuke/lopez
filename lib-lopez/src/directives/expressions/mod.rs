pub mod parse;

mod aggregator;
mod extractor;
mod transformer;
mod value_ext;

pub use aggregator::AggregatorExpressionState;
pub use aggregator::{Aggregator, AggregatorExpression};
pub use extractor::{ExplodingExtractorExpression, ExtractorExpression};
pub use transformer::{ComparableRegex, Transformer, TransformerExpression, Type};

use std::fmt;

pub trait Parseable: Sized + Typed {
    fn parse(i: &str) -> nom::IResult<&str, Result<Self, String>>;
}

pub trait Typed: fmt::Display + fmt::Debug + PartialEq {
    fn type_of(&self) -> Result<Type, crate::Error>;
}

pub trait Extractable<E: Typed>: Copy {
    type Output;
    fn extract_with(self, extractor: &E) -> Self::Output;
}
