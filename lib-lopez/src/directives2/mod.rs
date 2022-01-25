use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "directives.pest"]
pub struct DirectivesParser;

