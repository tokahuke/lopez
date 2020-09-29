use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::digit1,
    combinator::{map, opt},
    multi::many0,
    number::complete::double,
    sequence::tuple,
    IResult,
};

use super::super::parse_common::*;
use super::Parseable;
use super::*;

fn transformer(i: &str) -> IResult<&str, Result<Transformer, String>> {
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
        map(tuple((tag_whitespace("equals"), double)), |(_, lhs)| {
            Ok(Transformer::Equals(lhs))
        }),
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
            |(_, _, transformer_expression, _)| Ok(Transformer::Filter(transformer_expression?)),
        ),
        map(tag("pretty"), |_| Ok(Transformer::Pretty)),
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
                    replacer,
                ))
            },
        ),
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

pub fn extractor_expression<P: Parseable>(
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

pub fn exploding_extractor_expression<P: Parseable>(
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

pub fn aggregator<P: Parseable>(i: &str) -> IResult<&str, Result<Aggregator<P>, String>> {
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

pub fn aggregator_expression<P: Parseable>(
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
