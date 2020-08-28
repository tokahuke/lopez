use scraper::ElementRef;
use serde_json::{to_value, Value};

use super::transformer::{Type, Transformer};

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Extractor {
    Name,
    Text,
    Html,
    InnerHtml,
    Attr(String),
}

impl Extractor {
    pub fn type_of(&self) -> Type {
        match self {
            Extractor::Name => Type::String,
            Extractor::Attr(_) => Type::String,
            Extractor::Html => Type::String,
            Extractor::InnerHtml => Type::String,
            Extractor::Text => Type::String,
        }
    }

    pub fn extract(&self, element_ref: ElementRef) -> Value {
        match self {
            Extractor::Name => to_value(element_ref.value().name()),
            Extractor::Attr(attr) => to_value(element_ref.value().attr(attr)),
            Extractor::Html => to_value(element_ref.html()),
            Extractor::InnerHtml => to_value(element_ref.inner_html()),
            Extractor::Text => to_value(element_ref.text().collect::<Vec<_>>().join(" ")),
        }
        .expect("can always serialize")
    }
}

#[derive(Debug, Clone)]
pub struct ExtractorExpression {
    pub extractor: Extractor,
    pub transformers: Vec<Transformer>,
}

impl ExtractorExpression {
    pub fn type_of(&self) -> Result<Type, crate::Error> {
        let mut typ = self.extractor.type_of();

        for transformer in &self.transformers {
            if let Some(return_type) = transformer.type_for(&typ) {
                typ = return_type;
            } else {
                return Err(crate::Error::TypeError(transformer.clone(), typ));
            }
        }

        Ok(typ)
    }

    pub fn extract(&self, element_ref: ElementRef) -> Value {
        let mut extracted = self.extractor.extract(element_ref);

        for transformer in &self.transformers {
            extracted = transformer.eval(&mut extracted);
        }

        extracted
    }
}
