use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::digit1,
    combinator::{map, opt},
    multi::{many0, separated_list0},
    number::complete::double,
    sequence::tuple,
    IResult,
};

use super::super::parse_common::*;
use super::Parseable;
use super::*;

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

#[test]
fn type_test() {
    assert_eq!(
        r#type("map[string,array[   bool ]]"),
        Ok(("", Type::Map(Box::new(Type::Array(Box::new(Type::Bool))))))
    );
}

fn transformer(i: &str) -> IResult<&str, Result<Transformer, String>> {
    // Multiple alts because too many elements in tuple for poor nom...
    alt((
        alt((
            map(tag("is-null"), |_| Ok(Transformer::IsNull)),
            map(tag("is-not-null"), |_| Ok(Transformer::IsNotNull)),
            map(tag("hash"), |_| Ok(Transformer::Hash)),
            map(tag("not"), |_| Ok(Transformer::Not)),
            map(tag("as-number"), |_| Ok(Transformer::AsNumber)),
            map(
                tuple((tag_whitespace("greater-than"), double)),
                |(_, lhs)| Ok(Transformer::GreaterThan(lhs)),
            ),
            map(
                tuple((tag_whitespace("lesser-than"), double)),
                |(_, lhs)| Ok(Transformer::LesserThan(lhs)),
            ),
            map(
                tuple((tag_whitespace("greater-or-equal"), double)),
                |(_, lhs)| Ok(Transformer::GreaterOrEqual(lhs)),
            ),
            map(
                tuple((tag_whitespace("lesser-or-equal"), double)),
                |(_, lhs)| Ok(Transformer::LesserOrEqual(lhs)),
            ),
            map(
                tuple((
                    tag_whitespace("between"),
                    trailing_whitespace(double),
                    tag_whitespace("and"),
                    double,
                )),
                |(_, low, _, high)| Ok(Transformer::Between(low, high)),
            ),
            map(tuple((tag_whitespace("equals"), double)), |(_, lhs)| {
                Ok(Transformer::Equals(lhs))
            }),
            map(
                tuple((
                    tag_whitespace("in"),
                    tag_whitespace("["),
                    separated_list0(tag_whitespace(","), double),
                    tag("]"),
                )),
                |(_, _, list, _)| Ok(Transformer::In(list.into_boxed_slice())),
            ),
        )),
        alt((
            map(tag("length"), |_| Ok(Transformer::Length)),
            map(tag("is-empty"), |_| Ok(Transformer::IsEmpty)),
            map(tuple((tag_whitespace("get"), digit1)), |(_, digits)| {
                Ok(Transformer::GetIdx(
                    digits.parse().map_err(|err| format!("{}", err))?,
                ))
            }),
            map(
                tuple((tag_whitespace("get"), escaped_string)),
                |(_, string)| Ok(Transformer::Get(string.into_boxed_str())),
            ),
            map(tag("flatten"), |_| Ok(Transformer::Flatten)),
            map(
                tuple((
                    tag_whitespace("each"),
                    tag_whitespace("("),
                    transformer_expression,
                    tag(")"),
                )),
                |(_, _, transformer_expression, _)| Ok(Transformer::Each(transformer_expression?)),
            ),
            map(
                tuple((
                    tag_whitespace("filter"),
                    tag_whitespace("("),
                    transformer_expression,
                    tag(")"),
                )),
                |(_, _, transformer_expression, _)| {
                    Ok(Transformer::Filter(transformer_expression?))
                },
            ),
            map(
                tuple((
                    tag_whitespace("any"),
                    tag_whitespace("("),
                    transformer_expression,
                    tag(")"),
                )),
                |(_, _, transformer_expression, _)| Ok(Transformer::Any(transformer_expression?)),
            ),
            map(
                tuple((
                    tag_whitespace("all"),
                    tag_whitespace("("),
                    transformer_expression,
                    tag(")"),
                )),
                |(_, _, transformer_expression, _)| Ok(Transformer::All(transformer_expression?)),
            ),
            map(tag("sort"), |_| Ok(Transformer::Sort)),
        )),
        alt((
            map(tag("as-string"), |_| Ok(Transformer::AsString)),
            map(tag("pretty"), |_| Ok(Transformer::Pretty)),
            map(
                tuple((tag_whitespace("equals"), escaped_string)),
                |(_, string)| Ok(Transformer::EqualsString(string.into_boxed_str())),
            ),
            map(
                tuple((
                    tag_whitespace("in"),
                    tag_whitespace("["),
                    separated_list0(tag_whitespace(","), escaped_string),
                    tag("]"),
                )),
                |(_, _, list, _)| {
                    Ok(Transformer::InStrings(
                        list.into_iter()
                            .map(|string| string.into_boxed_str())
                            .collect(),
                    ))
                },
            ),
            map(
                tuple((tag_whitespace("capture"), escaped_string)),
                |(_, regexp)| Ok(Transformer::Capture(ComparableRegex(regex(&regexp)?))),
            ),
            map(
                tuple((tag_whitespace("all-captures"), escaped_string)),
                |(_, regexp)| Ok(Transformer::AllCaptures(ComparableRegex(regex(&regexp)?))),
            ),
            map(
                tuple((tag_whitespace("matches"), escaped_string)),
                |(_, regexp)| Ok(Transformer::Matches(ComparableRegex(regex(&regexp)?))),
            ),
            map(
                tuple((
                    tag_whitespace("replace"),
                    trailing_whitespace(escaped_string),
                    tag_whitespace("with"),
                    escaped_string,
                )),
                |(_, regexp, _, replacer)| {
                    Ok(Transformer::Replace(
                        ComparableRegex(regex(&regexp)?),
                        replacer.into_boxed_str(),
                    ))
                },
            ),
        )),
    ))(i)
}

#[test]
fn transformer_test() {
    use regex::Regex;
    use std::str::FromStr;

    // No `PartialEq` for me.
    match transformer("capture \n\t \"$(:!?foo)*\"")
        .unwrap()
        .1
        .unwrap()
    {
        Transformer::Capture(ComparableRegex(regex)) => assert_eq!(
            Regex::from_str("$(:!?foo)*").unwrap().as_str(),
            regex.as_str()
        ),
        e => panic!("got {:?}", e),
    }
}

fn transformer_expression(i: &str) -> IResult<&str, Result<TransformerExpression, String>> {
    map(many0(trailing_whitespace(transformer)), |transformers| {
        Ok(TransformerExpression {
            transformers: {
                let mut transformers = transformers.into_iter().collect::<Result<Vec<_>, _>>()?;
                transformers.shrink_to_fit();
                transformers.into_boxed_slice()
            },
        })
    })(i)
}

pub fn extractor_expression<P: Parseable + Typed>(
    i: &str,
) -> IResult<&str, Result<ExtractorExpression<P>, String>> {
    map(
        tuple((trailing_whitespace(P::parse), transformer_expression)),
        |(extractor, transformer_expression)| {
            Ok(ExtractorExpression {
                extractor: extractor?,
                transformer_expression: transformer_expression?,
            })
        },
    )(i)
}

pub fn exploding_extractor_expression<P: Parseable + Typed>(
    i: &str,
) -> IResult<&str, Result<ExplodingExtractorExpression<P>, String>> {
    map(
        tuple((
            trailing_whitespace(extractor_expression::<P>),
            opt(tag("!explode")),
        )),
        |(extractor_expression, explodes)| {
            Ok(ExplodingExtractorExpression {
                explodes: explodes.is_some(),
                extractor_expression: extractor_expression?,
            })
        },
    )(i)
}

pub fn aggregator<P: Parseable + Typed>(i: &str) -> IResult<&str, Result<Aggregator<P>, String>> {
    alt((
        map(
            tuple((
                tag_whitespace("count"),
                tag_whitespace("("),
                trailing_whitespace(exploding_extractor_expression::<P>),
                tag(")"),
            )),
            |(_, _, extractor, _)| Ok(Aggregator::CountNotNull(extractor?)),
        ),
        map(tag("count"), |_| Ok(Aggregator::Count)),
        map(
            tuple((
                tag_whitespace("first"),
                tag_whitespace("("),
                trailing_whitespace(exploding_extractor_expression::<P>),
                tag(")"),
            )),
            |(_, _, extractor, _)| Ok(Aggregator::First(extractor?)),
        ),
        map(
            tuple((
                tag_whitespace("collect"),
                tag_whitespace("("),
                trailing_whitespace(exploding_extractor_expression::<P>),
                tag(")"),
            )),
            |(_, _, extractor, _)| Ok(Aggregator::Collect(extractor?)),
        ),
        map(
            tuple((
                tag_whitespace("distinct"),
                tag_whitespace("("),
                trailing_whitespace(exploding_extractor_expression::<P>),
                tag(")"),
            )),
            |(_, _, extractor, _)| Ok(Aggregator::Distinct(extractor?)),
        ),
        map(
            tuple((
                tag_whitespace("sum"),
                tag_whitespace("("),
                trailing_whitespace(exploding_extractor_expression::<P>),
                tag(")"),
            )),
            |(_, _, extractor, _)| Ok(Aggregator::Sum(extractor?)),
        ),
        map(
            tuple((
                tag_whitespace("group"),
                tag_whitespace("("),
                trailing_whitespace(exploding_extractor_expression::<P>),
                tag_whitespace(","),
                trailing_whitespace(aggregator_expression),
                tag(")"),
            )),
            |(_, _, extractor, _, aggregator, _)| {
                Ok(Aggregator::Group(extractor?, Box::new(aggregator?)))
            },
        ),
    ))(i)
}

pub fn aggregator_expression<P: Parseable + Typed>(
    i: &str,
) -> IResult<&str, Result<AggregatorExpression<P>, String>> {
    map(
        tuple((aggregator::<P>, whitespace, transformer_expression)),
        |(aggregator, _, transformer_expression)| {
            Ok(AggregatorExpression {
                aggregator: aggregator?,
                transformer_expression: transformer_expression?,
            })
        },
    )(i)
}
