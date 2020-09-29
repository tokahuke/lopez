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

pub enum Error {
    TypeError(String, Type),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::TypeError(thing, unexpected) => {
                write!(f, "type error: no type for `{}` of `{}`", thing, unexpected)
            }
        }
    }
}

pub trait Parseable: Sized + Typed {
    fn parse(i: &str) -> nom::IResult<&str, Result<Self, String>>;
}

pub trait Typed: fmt::Display + fmt::Debug + PartialEq {
    fn type_of(&self) -> Result<Type, Error>;
}

pub trait Extractable<E: Typed>: Copy {
    type Output;
    fn extract_with(self, extractor: &E) -> Self::Output;
}
