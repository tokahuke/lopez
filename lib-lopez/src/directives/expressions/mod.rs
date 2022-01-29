#![allow(dead_code)] // in preparation to extract this into its own crate.

pub mod parse;

mod aggregator;
mod extractor;
mod transformer;
mod value_ext;

pub use aggregator::AggregatorExpressionState;
pub use aggregator::{Aggregator, AggregatorExpression};
pub use extractor::{ExplodingExtractorExpression, ExtractorExpression};
pub use transformer::{ComparableRegex, Transformer, TransformerExpression};

use std::fmt;

use crate::Type;

#[derive(Debug)]
pub enum Error {
    TypeError(String, Type),
    ExpectedType {
        thing: String,
        got: Type,
        expected: Type,
    },
    NotExpectedType {
        thing: String,
        not_expected: Type,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::TypeError(thing, unexpected) => {
                write!(f, "type error: no type for `{}` of `{}`", thing, unexpected)
            }
            Error::ExpectedType {
                thing,
                expected,
                got,
            } => write!(
                f,
                "type error: expected {} for {} and got {}",
                expected, thing, got
            ),
            Error::NotExpectedType {
                thing,
                not_expected,
            } => write!(f, "type error: not expected {} for {}", not_expected, thing,),
        }
    }
}

pub trait Parseable: Sized {
    fn parse(i: &str) -> nom::IResult<&str, Result<Self, String>>;
}

pub trait Typed: fmt::Display + fmt::Debug + PartialEq {
    fn type_of(&self) -> Result<Type, Error>;
}

pub trait Extractable<E: Typed>: Copy {
    type Output;
    fn extract_with(self, extractor: &E) -> Self::Output;
}
