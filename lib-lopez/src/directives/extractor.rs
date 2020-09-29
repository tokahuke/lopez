use scraper::ElementRef;
use serde_json::{Map, Value};
use std::fmt;

use super::expressions::{Extractable, ExtractorExpression, Type, Typed};

#[derive(Debug, PartialEq)]
pub enum Extractor {
    Name,
    Text,
    Html,
    InnerHtml,
    Attr(Box<str>),
    Attrs,
    Classes,
    Id,
    Parent(Box<ExtractorExpression<Self>>),
    Children(Box<ExtractorExpression<Self>>),
    SelectAny(Box<ExtractorExpression<Self>>, scraper::Selector),
    SelectAll(Box<ExtractorExpression<Self>>, scraper::Selector),
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

impl Typed for Extractor {
    fn type_of(&self) -> Result<Type, crate::Error> {
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
}

impl<'a> Extractable<Extractor> for ElementRef<'a> {
    type Output = Value;

    #[inline(always)]
    fn extract_with(self, extractor: &Extractor) -> Value {
        match extractor {
            Extractor::Name => self.value().name().into(),
            Extractor::Html => self.html().into(),
            Extractor::InnerHtml => self.inner_html().into(),
            Extractor::Text => self.text().collect::<Vec<_>>().join(" ").into(),
            Extractor::Attr(attr) => self
                .value()
                .attr(attr)
                .map(|value| value.into())
                .unwrap_or(Value::Null),
            Extractor::Attrs => self
                .value()
                .attrs()
                .map(|(key, value)| (key.to_owned(), value.to_owned().into()))
                .collect::<Map<_, _>>()
                .into(),
            Extractor::Classes => self.value().classes().collect::<Vec<_>>().into(),
            Extractor::Id => self.value().id().map(|id| id.into()).unwrap_or(Value::Null),
            Extractor::Parent(parent) => self
                .parent()
                .and_then(ElementRef::wrap)
                .map(|element_ref| element_ref.extract_with(parent.as_ref()))
                .unwrap_or(Value::Null),
            Extractor::Children(children) => self
                .children()
                .filter_map(ElementRef::wrap)
                .map(|element_ref| element_ref.extract_with(children.as_ref()))
                .collect::<Vec<_>>()
                .into(),
            Extractor::SelectAny(extractor, selector) => self
                .select(selector)
                .next()
                .map(|element_ref| element_ref.extract_with(extractor.as_ref()))
                .unwrap_or(Value::Null),
            Extractor::SelectAll(extractor, selector) => self
                .select(selector)
                .map(|element_ref| element_ref.extract_with(extractor.as_ref()))
                .collect::<Vec<_>>()
                .into(),
        }
    }
}
