//! All modules are dependent on a notion of a JSON-like type. Therefore, this impl is extracted to this module.

use nom::{
    branch::alt, bytes::complete::tag, character::complete::multispace1, combinator::map,
    multi::many0, sequence::tuple, IResult,
};
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
        nom::combinator::all_consuming(trailing_whitespace(r#type))(s)
            .map(|(_, r#type)| r#type)
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

/// Defines what is whitespace:
pub fn whitespace(i: &str) -> IResult<&str, ()> {
    map(many0(multispace1), |_| ())(i)
}

pub fn trailing_whitespace<'a, F, T>(f: F) -> impl FnMut(&'a str) -> IResult<&'a str, T>
where
    F: FnMut(&'a str) -> IResult<&'a str, T>,
{
    map(tuple((f, whitespace)), |(t, _)| t)
}

#[allow(clippy::needless_lifetimes)] // not that needless...
pub fn tag_whitespace<'a>(tag_val: &'a str) -> impl FnMut(&'a str) -> IResult<&'a str, &'a str> {
    trailing_whitespace(tag(tag_val))
}

pub fn r#type(i: &str) -> IResult<&str, Type> {
    alt((
        map(tag("any"), |_| Type::Any),
        map(tag("bool"), |_| Type::Bool),
        map(tag("number"), |_| Type::Number),
        map(tag("string"), |_| Type::String),
        map(
            tuple((
                tag_whitespace("array"),
                tag_whitespace("["),
                trailing_whitespace(r#type),
                tag("]"),
            )),
            |(_, _, r#type, _)| Type::Array(Box::new(r#type)),
        ),
        map(
            tuple((
                tag_whitespace("map"),
                tag_whitespace("["),
                tag_whitespace("string"),
                tag_whitespace(","),
                trailing_whitespace(r#type),
                tag_whitespace("]"),
            )),
            |(_, _, _, _, r#type, _)| Type::Map(Box::new(r#type)),
        ),
    ))(i)
}
