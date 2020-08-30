use scraper::ElementRef;
use serde_json::{Map, Value};
use std::fmt;

use super::transformer::{TransformerExpression, Type};

#[derive(Debug, Clone)]
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
    Parent(Box<ExtractorExpression>),
    Children(Box<ExtractorExpression>),
    SelectAny(Box<ExtractorExpression>, scraper::Selector),
    SelectAll(Box<ExtractorExpression>, scraper::Selector),
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
            Extractor::Parent(parent) => write!(f, "parent({})", parent),
            Extractor::Children(children) => write!(f, "children({})", children),
            Extractor::SelectAny(select_any, extractor) => {
                write!(f, "select-any({}, {:?})", select_any, extractor)
            } // TODO: display correctly.
            Extractor::SelectAll(select_all, extractor) => {
                write!(f, "select-any({}, {:?})", select_all, extractor)
            } // TODO: display correctly.
        }
    }
}

impl Extractor {
    pub fn type_of(&self) -> Result<Type, crate::Error> {
        Ok(match self {
            Extractor::Name => Type::String,
            Extractor::Html => Type::String,
            Extractor::InnerHtml => Type::String,
            Extractor::Text => Type::String,
            Extractor::Attr(_) => Type::String,
            Extractor::Attrs => Type::Map(Box::new(Type::String)),
            Extractor::Classes => Type::Array(Box::new(Type::String)),
            Extractor::Id => Type::String,
            Extractor::Parent(parent) => parent.type_of()?,
            Extractor::Children(children) => Type::Array(Box::new(children.type_of()?)),
            Extractor::SelectAny(extractor, _) => extractor.type_of()?,
            Extractor::SelectAll(extractor, _) => Type::Array(Box::new(extractor.type_of()?)),
        })
    }

    pub fn extract(&self, element_ref: ElementRef) -> Value {
        match self {
            Extractor::Name => element_ref.value().name().into(),
            Extractor::Html => element_ref.html().into(),
            Extractor::InnerHtml => element_ref.inner_html().into(),
            Extractor::Text => element_ref.text().collect::<Vec<_>>().join(" ").into(),
            Extractor::Attr(attr) => element_ref
                .value()
                .attr(attr)
                .map(|value| value.into())
                .unwrap_or(Value::Null),
            Extractor::Attrs => element_ref
                .value()
                .attrs()
                .map(|(key, value)| (key.to_owned(), value.to_owned().into()))
                .collect::<Map<_, _>>()
                .into(),
            Extractor::Classes => element_ref.value().classes().collect::<Vec<_>>().into(),
            Extractor::Id => element_ref
                .value()
                .id()
                .map(|id| id.into())
                .unwrap_or(Value::Null),
            Extractor::Parent(parent) => element_ref
                .parent()
                .and_then(|node_ref| ElementRef::wrap(node_ref))
                .map(|element_ref| parent.extract(element_ref))
                .unwrap_or(Value::Null),
            Extractor::Children(children) => element_ref
                .children()
                .filter_map(|node_ref| ElementRef::wrap(node_ref))
                .map(|element_ref| children.extract(element_ref))
                .collect::<Vec<_>>()
                .into(),
            Extractor::SelectAny(extractor, selector) => element_ref
                .select(selector)
                .next()
                .map(|element_ref| extractor.extract(element_ref))
                .unwrap_or(Value::Null),
            Extractor::SelectAll(extractor, selector) => element_ref
                .select(selector)
                .map(|element_ref| extractor.extract(element_ref))
                .collect::<Vec<_>>()
                .into(),
        }
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
            .type_for(&self.extractor.type_of()?)
    }

    pub fn extract(&self, element_ref: ElementRef) -> Value {
        self.transformer_expression
            .eval(self.extractor.extract(element_ref))
    }
}
