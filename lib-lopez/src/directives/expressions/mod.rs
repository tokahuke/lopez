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

use serde_derive::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

impl FromStr for Type {
    type Err = String;
    fn from_str(s: &str) -> Result<Type, String> {
        super::parse_utils::ParseError::map_iresult(
            s,
            nom::combinator::all_consuming(super::parse_common::trailing_whitespace(parse::r#type))(
                s,
            ),
        )
        .map_err(|err| err.to_string())
    }
}

impl Type {
    pub fn _is_array(&self) -> bool {
        if let Type::Array(_) = self {
            true
        } else {
            false
        }
    }

    pub fn is_map(&self) -> bool {
        if let Type::Map(_) = self {
            true
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub enum Error {
    TypeError(String, Type),
    ExpectedType {
        thing: String,
        got: Type,
        expected: Type,
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
