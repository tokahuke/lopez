use scraper::ElementRef;
use serde_json::{to_value, Map, Value};
use std::fmt;

use super::transformer::{TransformerExpression, Type};

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Extractor {
    Name,
    Text,
    Html,
    InnerHtml,
    Attr(String),
    Attrs,
    Classes,
    Id,
}

impl fmt::Display for Extractor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Extractor::Name => write!(f, "name"),
            Extractor::Text => write!(f, "text"),
            Extractor::Html => write!(f, "html"),
            Extractor::InnerHtml => write!(f, "inner-html"),
            Extractor::Attr(attr) => write!(f, "attr \"{:?}\"", attr.replace('\"', "\\\"")),
            Extractor::Attrs => write!(f, "attrs"),
            Extractor::Classes => write!(f, "classes"),
            Extractor::Id => write!(f, "id"),
        }
    }
}

impl Extractor {
    pub fn type_of(&self) -> Type {
        match self {
            Extractor::Name => Type::String,
            Extractor::Html => Type::String,
            Extractor::InnerHtml => Type::String,
            Extractor::Text => Type::String,
            Extractor::Attr(_) => Type::String,
            Extractor::Attrs => Type::Map(Box::new(Type::String)),
            Extractor::Classes => Type::Array(Box::new(Type::String)),
            Extractor::Id => Type::String,
        }
    }

    pub fn extract(&self, element_ref: ElementRef) -> Value {
        match self {
            Extractor::Name => to_value(element_ref.value().name()),
            Extractor::Html => to_value(element_ref.html()),
            Extractor::InnerHtml => to_value(element_ref.inner_html()),
            Extractor::Text => to_value(element_ref.text().collect::<Vec<_>>().join(" ")),
            Extractor::Attr(attr) => to_value(element_ref.value().attr(attr)),
            Extractor::Attrs => to_value(
                element_ref
                    .value()
                    .attrs()
                    .map(|(key, value)| (key.to_owned(), value.to_owned().into()))
                    .collect::<Map<_, _>>(),
            ),
            Extractor::Classes => to_value(element_ref.value().classes().collect::<Vec<_>>()),
            Extractor::Id => to_value(element_ref.value().id()),
        }
        .expect("can always serialize")
    }
}

#[derive(Debug, Clone)]
pub struct ExtractorExpression {
    pub extractor: Extractor,
    pub transformer_expression: TransformerExpression,
}

impl fmt::Display for ExtractorExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.transformer_expression.is_empty() {
            write!(f, "{}", self.extractor)
        } else {
            write!(f, "{} ", self.extractor)?;
            write!(f, "{}", self.transformer_expression)
        }
    }
}

impl ExtractorExpression {
    pub fn type_of(&self) -> Result<Type, crate::Error> {
        self.transformer_expression
            .type_for(&self.extractor.type_of())
    }

    pub fn extract(&self, element_ref: ElementRef) -> Value {
        self.transformer_expression
            .eval(self.extractor.extract(element_ref))
    }
}
