use serde_json::Value;
use smallvec::SmallVec;
use std::fmt;

use super::transformer::{TransformerExpression, Type};
use super::{Extractable, Typed};

#[derive(Debug, PartialEq)]
pub struct ExtractorExpression<E: Typed> {
    pub extractor: E,
    pub transformer_expression: TransformerExpression,
}

impl<E: Typed> fmt::Display for ExtractorExpression<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.transformer_expression.is_empty() {
            write!(f, "{}", self.extractor)
        } else {
            write!(f, "{} ", self.extractor)?;
            write!(f, "{}", self.transformer_expression)
        }
    }
}

impl<E: Typed> Typed for ExtractorExpression<E> {
    fn type_of(&self) -> Result<Type, crate::Error> {
        self.transformer_expression
            .type_for(&self.extractor.type_of()?)
    }
}

impl<T, E: Typed> Extractable<ExtractorExpression<E>> for T
where
    T: Extractable<E, Output = Value>,
{
    type Output = Value;

    fn extract_with(self, extractor_expr: &ExtractorExpression<E>) -> Value {
        extractor_expr
            .transformer_expression
            .eval(self.extract_with(&extractor_expr.extractor))
    }
}

#[derive(Debug, PartialEq)]
pub struct ExplodingExtractorExpression<E: Typed> {
    pub explodes: bool,
    pub extractor_expression: ExtractorExpression<E>,
}

impl<E: Typed> fmt::Display for ExplodingExtractorExpression<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.explodes {
            write!(f, "{} !explode", self.extractor_expression)
        } else {
            write!(f, "{}", self.extractor_expression)
        }
    }
}

impl<E: Typed> Typed for ExplodingExtractorExpression<E> {
    fn type_of(&self) -> Result<Type, crate::Error> {
        let raw = self.extractor_expression.type_of()?;

        if self.explodes {
            if let Type::Array(inner) = raw {
                Ok(Type::clone(&inner))
            } else {
                Err(crate::Error::TypeError("!explode".to_owned(), raw))
            }
        } else {
            Ok(raw)
        }
    }
}

impl<T, E: Typed> Extractable<ExplodingExtractorExpression<E>> for T
where
    T: Extractable<ExtractorExpression<E>, Output = Value>,
{
    type Output = SmallVec<[Value; 1]>;

    #[inline(always)]
    fn extract_with(
        self,
        extractor_expr: &ExplodingExtractorExpression<E>,
    ) -> SmallVec<[Value; 1]> {
        let extracted = self.extract_with(&extractor_expr.extractor_expression);
        if extractor_expr.explodes {
            if let Value::Array(array) = extracted {
                SmallVec::from_vec(array)
            } else {
                todo!()
            }
        } else {
            SmallVec::from_buf([extracted])
        }
    }
}
