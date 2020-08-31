use nom::{
    branch::alt,
    bytes::complete::{escaped, is_not, tag},
    character::complete::{anychar, digit1, multispace1},
    combinator::{all_consuming, map, map_res, not, opt},
    multi::{many0, separated_list},
    number::complete::double,
    sequence::{delimited, tuple},
    IResult,
};
use regex::Regex;
use std::collections::HashMap;
use std::str::FromStr;
use url::Url;

use super::*;

/// Defines end of file (lol!):
fn eof(i: &str) -> IResult<&str, ()> {
    if i.is_empty() {
        Ok((i, ()))
    } else {
        Err(nom::Err::Error((i, nom::error::ErrorKind::IsA)))
    }
}

/// Defines a comment line. This is the only kind of comment.
fn comment(i: &str) -> IResult<&str, ()> {
    map(
        delimited(tag("//"), is_not("\n"), alt((map(tag("\n"), |_| ()), eof))),
        |_| (),
    )(i)
}

#[test]
fn comment_test() {
    assert_eq!(comment("// foo!\n").unwrap(), ("", ()));
}

/// Defines what is whitespace:
fn whitespace(i: &str) -> IResult<&str, ()> {
    map(many0(alt((comment, map(multispace1, |_| ())))), |_| ())(i)
}

#[test]
fn whitespace_test() {
    assert_eq!(
        whitespace("//foo! \n\n\t  // bar! \n  \n\rnhé!").unwrap(),
        ("nhé!", ())
    );
    assert_eq!(whitespace("").unwrap(), ("", ())); // is this behavior wise?
}

fn trailing_whitespace<'a, F, T>(f: F) -> impl Fn(&'a str) -> IResult<&'a str, T>
where
    F: Fn(&'a str) -> IResult<&'a str, T>,
{
    map(tuple((f, whitespace)), |(t, _)| t)
}

fn tag_whitespace<'a>(tag_val: &'a str) -> impl Fn(&'a str) -> IResult<&'a str, &'a str> {
    trailing_whitespace(tag(tag_val))
}

fn tags_whitespace<'a>(tag_vals: &'a [&'a str]) -> impl Fn(&'a str) -> IResult<&'a str, ()> {
    move |mut i: &str| {
        for &tag_val in tag_vals {
            let (matched, _) = tag_whitespace(tag_val)(i)?;
            i = matched;
        }

        Ok((i, ()))
    }
}

#[test]
fn tag_whitespace_test() {
    assert_eq!(tag_whitespace("foo")("foo  // bar\n"), Ok(("", "foo")));
    assert_eq!(tag_whitespace("foo")("foo"), Ok(("", "foo")));
}

fn escaped_string(i: &str) -> IResult<&str, String> {
    let (i, escaped) = delimited(
        tag("\""),
        escaped(is_not(r#"\""#), '\\', anychar),
        tag("\""),
    )(i)?;

    let mut unescaped = String::with_capacity(escaped.len());
    let mut is_escaped = false;

    for ch in escaped.chars() {
        match ch {
            '"' if is_escaped => {
                is_escaped = false;
                unescaped.push('"')
            }
            ch if is_escaped => {
                is_escaped = false;
                unescaped.extend(&['\\', ch])
            }
            '\\' => {
                is_escaped = true;
            }
            ch => {
                is_escaped = false;
                unescaped.push(ch)
            }
        }
    }

    Ok((i, unescaped))
}

#[test]
fn escaped_string_test() {
    assert_eq!(
        escaped_string(
            r#""foo\"
bar"ho-ho"#
        ),
        Ok((
            "ho-ho",
            r#"foo"
bar"#
                .to_owned()
        ))
    );
    assert_eq!(escaped_string("\"foo\""), Ok(("", "foo".to_owned())));
    assert_eq!(
        escaped_string("\"foo\\.bar\""),
        Ok(("", "foo\\.bar".to_owned()))
    );
}

fn regex(parsed: &str) -> Result<Regex, String> {
    Regex::from_str(parsed).map_err(|err| format!("{}", err))
}

fn identifier(i: &str) -> IResult<&str, &str> {
    is_not("\\/:;.()[]{}\'\" \n\t\r\0")(i)
}

#[test]
fn identifier_test() {
    assert_eq!(
        identifier("a-very_funnyIdentifier_SCREAMING_$123"),
        Ok(("", "a-very_funnyIdentifier_SCREAMING_$123"))
    );
}

fn identified_value<'a, F, T>(f: F) -> impl Fn(&'a str) -> IResult<&'a str, (&'a str, T)>
where
    F: Fn(&'a str) -> IResult<&'a str, T>,
{
    map(
        tuple((
            trailing_whitespace(identifier),
            tag_whitespace(":"),
            trailing_whitespace(f),
            tag(";"),
        )),
        |(identifier, _, value, _)| (identifier, value),
    )
}

#[test]
fn identified_value_test() {
    assert_eq!(
        identified_value(is_not(";"))("foo: bar;"),
        Ok(("", ("foo", "bar")))
    );
}

fn block<'a, He, Va, H, V>(head: He, value: Va) -> impl Fn(&'a str) -> IResult<&'a str, (H, Vec<V>)>
where
    He: Fn(&'a str) -> IResult<&'a str, H>,
    Va: Fn(&'a str) -> IResult<&'a str, V>,
{
    tuple((
        trailing_whitespace(head),
        alt((
            delimited(
                tag_whitespace("{"),
                many0(trailing_whitespace(value)),
                tag("}"),
            ),
            map(tag(";"), |_| vec![]),
        )),
    ))
}

#[test]
fn block_test() {
    assert_eq!(
        block(identifier, identified_value(is_not(";")))("Foo{ foo: bar; baz: qux; }"),
        Ok(("", ("Foo", vec![("foo", "bar"), ("baz", "qux")]))),
    );
}

fn css_selector(
    boundary_hint: char,
) -> impl Fn(&str) -> IResult<&str, Result<scraper::Selector, String>> {
    move |i: &str| {
        let mut level = 0;
        let mut idx = 0;

        while idx < i.len() && (level != 0 || !i[idx..].starts_with(boundary_hint)) {
            if i[idx..].starts_with('[') {
                level += 1;
            } else if i[idx..].starts_with(']') {
                level -= 1;
            }

            idx += 1;
        }

        if idx == 0 {
            Err(nom::Err::Error((i, nom::error::ErrorKind::IsA)))
        } else {
            Ok((
                &i[idx..],
                scraper::Selector::parse(&i[..idx]).map_err(|err| format!("{:?}", err)),
            ))
        }
    }
}

#[test]
fn css_selector_test() {
    let selector = scraper::Selector::parse("div > a + button[foo$=\"bar{\" i]").unwrap();
    assert_eq!(
        css_selector('{')("div > a + button[foo$=\"bar{\" i]{ haha-hoho!"),
        Ok(("{ haha-hoho!", Ok(selector)))
    );
}

fn transformer(i: &str) -> IResult<&str, Result<Transformer, String>> {
    alt((
        map(tag("is-null"), |_| Ok(Transformer::IsNull)),
        map(tag("is-not-null"), |_| Ok(Transformer::IsNotNull)),
        map(tag("hash"), |_| Ok(Transformer::Hash)),
        map(tag("not"), |_| Ok(Transformer::Not)),
        map(tag("as-number"), |_| Ok(Transformer::AsNumber)),
        map(tuple((tag_whitespace("greater-than"), double)), |(_, lhs)| {
            Ok(Transformer::GreaterThan(lhs))
        }),
        map(tuple((tag_whitespace("lesser-than"), double)), |(_, lhs)| {
            Ok(Transformer::LesserThan(lhs))
        }),
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
    ))(i)
}

#[test]
fn transformer_test() {
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

fn extractor(i: &str) -> IResult<&str, Result<Extractor, String>> {
    alt((
        map(tag("name"), |_| Ok(Extractor::Name)),
        map(tag("text"), |_| Ok(Extractor::Text)),
        map(tag("html"), |_| Ok(Extractor::Html)),
        map(tag("inner-html"), |_| Ok(Extractor::InnerHtml)),
        map(tag("attrs"), |_| Ok(Extractor::Attrs)),
        map(tag("classes"), |_| Ok(Extractor::Classes)),
        map(tag("id"), |_| Ok(Extractor::Id)),
        map(
            tuple((tag_whitespace("attr"), escaped_string)),
            |(_, attr)| Ok(Extractor::Attr(attr.into_boxed_str())),
        ),
        map(
            tuple((
                tag_whitespace("parent"),
                tag_whitespace("("),
                trailing_whitespace(extractor_expression),
                tag(")"),
            )),
            |(_, _, extractor, _)| Ok(Extractor::Parent(Box::new(extractor?))),
        ),
        map(
            tuple((
                tag_whitespace("children"),
                tag_whitespace("("),
                trailing_whitespace(extractor_expression),
                tag(")"),
            )),
            |(_, _, extractor, _)| Ok(Extractor::Children(Box::new(extractor?))),
        ),
        map(
            tuple((
                tag_whitespace("select-any"),
                tag_whitespace("("),
                trailing_whitespace(extractor_expression),
                tag_whitespace(","),
                css_selector(')'),
                tag(")"),
            )),
            |(_, _, extractor, _, selector, _)| {
                Ok(Extractor::SelectAny(Box::new(extractor?), selector?))
            },
        ),
        map(
            tuple((
                tag_whitespace("select-all"),
                tag_whitespace("("),
                trailing_whitespace(extractor_expression),
                tag_whitespace(","),
                css_selector(')'),
                tag(")"),
            )),
            |(_, _, extractor, _, selector, _)| {
                Ok(Extractor::SelectAll(Box::new(extractor?), selector?))
            },
        ),
    ))(i)
}

#[test]
fn extractor_test() {
    assert_eq!(extractor("name"), Ok(("", Ok(Extractor::Name))));
    assert_eq!(
        extractor("attr \"foo\""),
        Ok(("", Ok(Extractor::Attr("foo".to_owned().into_boxed_str()))))
    );
    assert_eq!(extractor("inner-html"), Ok(("", Ok(Extractor::InnerHtml))));
    assert_eq!(
        extractor("select-all(text pretty, li)"),
        Ok((
            "",
            Ok(Extractor::SelectAll(
                Box::new(ExtractorExpression {
                    extractor: Extractor::Text,
                    transformer_expression: TransformerExpression {
                        transformers: vec![Transformer::Pretty],
                    },
                }),
                scraper::Selector::parse("li").unwrap()
            ),)
        ))
    );
}

fn extractor_expression(i: &str) -> IResult<&str, Result<ExtractorExpression, String>> {
    map(
        tuple((trailing_whitespace(extractor), transformer_expression)),
        |(extractor, transformer_expression)| {
            Ok(ExtractorExpression {
                extractor: extractor?,
                transformer_expression: transformer_expression?,
            })
        },
    )(i)
}

#[test]
fn extractor_expression_test() {
    assert_eq!(
        extractor_expression("attr \"src\" capture \"[0-9]+\""),
        Ok((
            "",
            Ok(ExtractorExpression {
                extractor: Extractor::Attr("src".to_owned().into_boxed_str()),
                transformer_expression: TransformerExpression {
                    transformers: vec![Transformer::Capture(ComparableRegex(
                        Regex::from_str("[0-9]+").unwrap()
                    ))]
                },
            })
        ))
    );
}

fn exploding_extractor_expression(
    i: &str,
) -> IResult<&str, Result<ExplodingExtractorExpression, String>> {
    map(
        tuple((
            trailing_whitespace(extractor_expression),
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

#[test]
fn exploding_extractor_expression_test() {
    assert_eq!(
        exploding_extractor_expression("attr \"src\" all-captures \"[0-9]+\" !explode"),
        Ok((
            "",
            Ok(ExplodingExtractorExpression {
                explodes: true,
                extractor_expression: ExtractorExpression {
                    extractor: Extractor::Attr("src".to_owned().into_boxed_str()),
                    transformer_expression: TransformerExpression {
                        transformers: vec![Transformer::AllCaptures(ComparableRegex(
                            Regex::from_str("[0-9]+").unwrap()
                        ))]
                    },
                }
            })
        ))
    );
}

fn aggregator(i: &str) -> IResult<&str, Result<Aggregator, String>> {
    alt((
        map(
            tuple((
                tag_whitespace("count"),
                tag_whitespace("("),
                trailing_whitespace(exploding_extractor_expression),
                tag(")"),
            )),
            |(_, _, extractor, _)| Ok(Aggregator::CountNotNull(extractor?)),
        ),
        map(tag("count"), |_| Ok(Aggregator::Count)),
        map(
            tuple((
                tag_whitespace("first"),
                tag_whitespace("("),
                trailing_whitespace(exploding_extractor_expression),
                tag(")"),
            )),
            |(_, _, extractor, _)| Ok(Aggregator::First(extractor?)),
        ),
        map(
            tuple((
                tag_whitespace("collect"),
                tag_whitespace("("),
                trailing_whitespace(exploding_extractor_expression),
                tag(")"),
            )),
            |(_, _, extractor, _)| Ok(Aggregator::Collect(extractor?)),
        ),
        map(
            tuple((
                tag_whitespace("distinct"),
                tag_whitespace("("),
                trailing_whitespace(exploding_extractor_expression),
                tag(")"),
            )),
            |(_, _, extractor, _)| Ok(Aggregator::Distinct(extractor?)),
        ),
        map(
            tuple((
                tag_whitespace("sum"),
                tag_whitespace("("),
                trailing_whitespace(exploding_extractor_expression),
                tag(")"),
            )),
            |(_, _, extractor, _)| Ok(Aggregator::Sum(extractor?)),
        ),
        map(
            tuple((
                tag_whitespace("group"),
                tag_whitespace("("),
                trailing_whitespace(exploding_extractor_expression),
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

#[test]
fn aggregator_test() {
    // No `PartialEq` for me.
    assert_eq!(
        aggregator("first(text capture \n\t \"$(:!?foo)*\")"),
        Ok((
            "",
            Ok(Aggregator::First(ExplodingExtractorExpression {
                explodes: false,
                extractor_expression: ExtractorExpression {
                    extractor: Extractor::Text,
                    transformer_expression: TransformerExpression {
                        transformers: vec![Transformer::Capture(ComparableRegex(
                            Regex::from_str("$(:!?foo)*").unwrap()
                        ))],
                    },
                },
            }))
        ))
    )
}

fn aggregator_expression(i: &str) -> IResult<&str, Result<AggregatorExpression, String>> {
    map(
        tuple((aggregator, whitespace, transformer_expression)),
        |(aggregator, _, transformer_expression)| {
            Ok(AggregatorExpression {
                aggregator: aggregator?,
                transformer_expression: transformer_expression?,
            })
        },
    )(i)
}

#[test]
fn aggregator_expression_test() {
    assert_eq!(
        aggregator_expression("first(text capture \n\t \"$(:!?foo)*\") length "),
        Ok((
            "",
            Ok(AggregatorExpression {
                aggregator: Aggregator::First(ExplodingExtractorExpression {
                    explodes: false,
                    extractor_expression: ExtractorExpression {
                        extractor: Extractor::Text,
                        transformer_expression: TransformerExpression {
                            transformers: vec![Transformer::Capture(ComparableRegex(
                                Regex::from_str("$(:!?foo)*").unwrap()
                            )),],
                        },
                    },
                }),
                transformer_expression: TransformerExpression {
                    transformers: vec![Transformer::Length,],
                },
            })
        ))
    )
}

fn in_directive(i: &str) -> IResult<&str, Result<Regex, String>> {
    map(
        tuple((tag_whitespace("in"), escaped_string)),
        |(_, parsed)| regex(&parsed),
    )(i)
}

#[test]
fn in_directive_test() {
    assert_eq!(
        in_directive("in \"$(:?!foo)*\"")
            .unwrap()
            .1
            .unwrap()
            .as_str(),
        Regex::from_str("$(:?!foo)*").unwrap().as_str()
    );
}

fn string_directive(
    directive_tags: &'static [&'static str],
) -> impl Fn(&str) -> IResult<&str, String> {
    move |i: &str| {
        map(
            tuple((
                tags_whitespace(directive_tags),
                trailing_whitespace(escaped_string),
                tag(";"),
            )),
            |(_, string, _)| string,
        )(i)
    }
}

#[test]
fn string_directive_test() {
    assert_eq!(
        string_directive(&["foo"])("foo \"bar\"  ;"),
        Ok(("", "bar".to_owned()))
    );
}

fn flag_directive(directive_tags: &'static [&'static str]) -> impl Fn(&str) -> IResult<&str, ()> {
    move |i: &str| {
        map(
            tuple((tags_whitespace(directive_tags), tag(";"))),
            |(_, _)| (),
        )(i)
    }
}

#[test]
fn flag_directive_test() {
    assert_eq!(flag_directive(&["foo"])("foo;"), Ok(("", ())));
}

#[derive(Debug)]
pub struct RuleSet {
    pub in_page: Option<Regex>,
    pub selector: scraper::Selector,
    pub aggregators: HashMap<String, AggregatorExpression>,
}

fn rule_set(i: &str) -> IResult<&str, Result<RuleSet, String>> {
    map(
        block(
            tuple((
                tag_whitespace("select"),
                opt(trailing_whitespace(in_directive)),
                css_selector('{'),
            )),
            identified_value(aggregator_expression),
        ),
        |((_, in_page, selector), aggregator_list)| {
            let mut aggregators = HashMap::new();

            for (identifier, aggregator) in aggregator_list {
                if aggregators.contains_key(identifier) {
                    return Err(format!("rule `{}` defined more than once", identifier));
                }

                aggregators.insert(identifier.to_owned(), aggregator?);
            }

            Ok(RuleSet {
                in_page: in_page.transpose()?,
                selector: selector?,
                aggregators,
            })
        },
    )(i)
}

#[test]
fn rule_set_test() {
    rule_set("select td > a[href^=\"https\"] { foo: first ( text ) ; }")
        .unwrap()
        .1
        .unwrap();
    rule_set("select ul { list: group(text, first(text pretty)); }")
        .unwrap()
        .1
        .unwrap();
    rule_set("select ul { list: collect(select-all(text, li) pretty); }")
        .unwrap()
        .1
        .unwrap();
}

#[derive(Debug, PartialEq)]
pub struct Module {
    pub path: String,
}

fn module(i: &str) -> IResult<&str, Module> {
    map(string_directive(&["import"]), |path| Module { path })(i)
}

#[test]
fn module_test() {
    assert_eq!(
        module("import \"foo.bar\";"),
        Ok((
            "",
            Module {
                path: "foo.bar".to_owned()
            }
        ))
    );
}

fn seed(i: &str) -> IResult<&str, Result<Url, String>> {
    map(string_directive(&["seed"]), |seed| {
        seed.parse::<Url>().map_err(|err| err.to_string())
    })(i)
}

#[test]
fn seed_test() {
    assert_eq!(
        seed("seed \"https://example.foo/bar/baz\";"),
        Ok(("", Ok(Url::parse("https://example.foo/bar/baz").unwrap())))
    )
}

#[derive(Debug)]
pub enum Boundary {
    Allowed(Regex),
    Disallowed(Regex),
    Frontier(Regex),
    UseParam(String),
    IgnoreParam(String),
    UseAllParams,
}

fn boundary(i: &str) -> IResult<&str, Result<Boundary, String>> {
    alt((
        map(string_directive(&["allow"]), |allowed| {
            Ok(Boundary::Allowed(regex(&allowed)?))
        }),
        map(string_directive(&["disallow"]), |disallowed| {
            Ok(Boundary::Disallowed(regex(&disallowed)?))
        }),
        map(string_directive(&["frontier"]), |frontier| {
            Ok(Boundary::Frontier(regex(&frontier)?))
        }),
        map(string_directive(&["use", "param"]), |use_param| {
            Ok(Boundary::UseParam(use_param))
        }),
        map(flag_directive(&["use", "param", "*"]), |_| {
            Ok(Boundary::UseAllParams)
        }),
        map(string_directive(&["ignore", "param"]), |ignore_param| {
            Ok(Boundary::IgnoreParam(ignore_param))
        }),
    ))(i)
}

#[test]
fn boundary_test() {
    match boundary("allow \"^https?://example.foo/\";")
        .unwrap()
        .1
        .unwrap()
    {
        Boundary::Allowed(allowed) => assert_eq!(
            allowed.as_str(),
            Regex::from_str("^https?://example.foo/").unwrap().as_str()
        ),
        b => panic!("got {:?}", b),
    }

    match boundary("disallow \"^https?://example.foo/\";")
        .unwrap()
        .1
        .unwrap()
    {
        Boundary::Disallowed(allowed) => assert_eq!(
            allowed.as_str(),
            Regex::from_str("^https?://example.foo/").unwrap().as_str()
        ),
        b => panic!("got {:?}", b),
    }

    match boundary("frontier \"^https?://example.foo/\";")
        .unwrap()
        .1
        .unwrap()
    {
        Boundary::Frontier(allowed) => assert_eq!(
            allowed.as_str(),
            Regex::from_str("^https?://example.foo/").unwrap().as_str()
        ),
        b => panic!("got {:?}", b),
    }
}

fn literal(i: &str) -> IResult<&str, Value> {
    alt((
        map(escaped_string, |string| Value::String(string)),
        map_res(tuple((digit1, not(tag(".")))), |(number, _): (&str, ())| {
            number.parse::<u64>().map(|num| num.into())
        }),
        map_res(
            tuple((tag("-"), digit1, not(tag(".")))),
            |(_, number, _): (_, &str, _)| number.parse::<i64>().map(|num| (-num).into()),
        ),
        map(double, |number| number.into()),
        map(
            tuple((
                tag_whitespace("["),
                separated_list(tag_whitespace(","), literal),
                tag("]"),
            )),
            |(_, array, _)| Value::Array(array),
        ),
    ))(i)
}

#[test]
fn literal_test() {
    assert_eq!(
        literal("\"a string\""),
        Ok(("", Value::String("a string".to_owned())))
    );
    assert_eq!(literal("1.234"), Ok(("", 1.234.into())));
    assert_eq!(literal("1234"), Ok(("", 1234.into())));
    assert_eq!(literal("-1234"), Ok(("", (-1234).into())));
    assert_eq!(literal("-1234.0"), Ok(("", (-1234.0).into())));
    assert_eq!(literal("1234.0"), Ok(("", (1234.0).into())));
}

#[derive(Debug, PartialEq)]
pub struct SetVariable {
    pub name: String,
    pub value: Value,
}

fn set_variable(i: &str) -> IResult<&str, SetVariable> {
    map(
        tuple((
            tag_whitespace("set"),
            trailing_whitespace(identifier),
            tag_whitespace("="),
            trailing_whitespace(literal),
            tag(";"),
        )),
        |(_, name, _, value, _)| SetVariable {
            name: name.to_owned(),
            value,
        },
    )(i)
}

#[test]
fn set_variable_test() {
    assert_eq!(
        set_variable("set a_variable = \"a value\";"),
        Ok((
            "",
            SetVariable {
                name: "a_variable".to_owned(),
                value: Value::String("a value".to_owned())
            }
        ))
    );
    assert_eq!(
        set_variable("set a_variable = 1.234;"),
        Ok((
            "",
            SetVariable {
                name: "a_variable".to_owned(),
                value: 1.234.into(),
            }
        ))
    );
    assert_eq!(
        set_variable("set a_variable = 1234;"),
        Ok((
            "",
            SetVariable {
                name: "a_variable".to_owned(),
                value: 1234.into(),
            }
        ))
    );
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Item {
    Seed(Url),
    Boundary(Boundary),
    Module(Module),
    RuleSet(Arc<RuleSet>),
    SetVariable(SetVariable),
}

fn item(i: &str) -> IResult<&str, Result<Item, String>> {
    alt((
        map(rule_set, |rule_set| Ok(Item::RuleSet(Arc::new(rule_set?)))),
        map(module, |module| Ok(Item::Module(module))),
        map(seed, |seed| Ok(Item::Seed(seed?))),
        map(boundary, |boundary| Ok(Item::Boundary(boundary?))),
        map(set_variable, |set_variable| {
            Ok(Item::SetVariable(set_variable))
        }),
    ))(i)
}

#[test]
fn item_test() {
    // dbg!(item("select td > a[href^=\"https\"] { foo: first(text) length; }"))
    //     .unwrap()
    //     .1
    //     .unwrap();

    //     dbg!(item(
    //         r#"select html {
    //     ldv-num: first(
    //         html all-captures "(?m)^.*0\s*8\s*0\s*0\s*4\s*1\s*1\s*0\s*5\s*0.*$"
    //         each(get "0")
    //     );
    // }
    // "#
    //     ));
}

pub fn entrypoint(i: &str) -> IResult<&str, Result<Vec<Item>, String>> {
    all_consuming(map(
        tuple((whitespace, many0(trailing_whitespace(item)))),
        |(_, results)| results.into_iter().collect::<Result<Vec<_>, _>>(),
    ))(i)
}

#[test]
fn entrypoint_test() {
    dbg!(entrypoint(
        "select * { } set foo = \"bar\"; allow \"foo\";\n"
    ))
    .unwrap()
    .1
    .unwrap();
}
