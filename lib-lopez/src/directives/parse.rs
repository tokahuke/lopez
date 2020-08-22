use nom::{
    branch::alt,
    bytes::complete::{escaped, is_not, tag},
    character::complete::{multispace1, one_of},
    combinator::{all_consuming, map, opt},
    multi::{many0, separated_list},
    sequence::{delimited, tuple},
    IResult,
};
use regex::Regex;
use std::collections::HashMap;
use std::str::FromStr;
use url::Url;

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
        escaped(is_not("\""), '\\', one_of(r#""nt\"#)),
        tag("\""),
    )(i)?;

    let mut unescaped = String::with_capacity(escaped.len() - 2);
    let mut is_escaped = false;

    for ch in escaped.chars() {
        match ch {
            '\\' if is_escaped => unescaped.push('\\'),
            '"' if is_escaped => unescaped.push('"'),
            'n' if is_escaped => unescaped.push('\n'),
            't' if is_escaped => unescaped.push('\t'),
            '\\' => {
                is_escaped = true;
                continue;
            }
            ch => unescaped.push(ch),
        }

        is_escaped = false;
    }

    Ok((i, unescaped))
}

#[test]
fn escaped_string_test() {
    assert_eq!(
        escaped_string("\"foo\\t\\nbar\"ho-ho"),
        Ok(("ho-ho", "foo\t\nbar".to_owned()))
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

fn css_selector(i: &str, boundary_hint: char) -> IResult<&str, Result<scraper::Selector, String>> {
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

#[test]
fn css_selector_test() {
    let selector = scraper::Selector::parse("div > a + button[foo$=\"bar{\" i]").unwrap();
    assert_eq!(
        css_selector("div > a + button[foo$=\"bar{\" i]{ haha-hoho!", '{'),
        Ok(("{ haha-hoho!", Ok(selector)))
    );
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Extractor {
    Name,
    Text,
    Capture(Regex),
    AllCaptures(Regex),
    Html,
    InnerHtml,
    Hash,
    Attr(String),
}

#[derive(Debug, Clone)]
pub enum Aggregator {
    Count,
    First(Extractor),
    Collect(Extractor),
}

fn extractor(i: &str) -> IResult<&str, Result<Extractor, String>> {
    alt((
        map(tag("name"), |_| Ok(Extractor::Name)),
        map(tag("text"), |_| Ok(Extractor::Text)),
        map(
            tuple((tag_whitespace("capture"), escaped_string)),
            |(_, regexp)| Ok(Extractor::Capture(regex(&regexp)?)),
        ),
        map(
            tuple((tag_whitespace("all-captures"), escaped_string)),
            |(_, regexp)| Ok(Extractor::AllCaptures(regex(&regexp)?)),
        ),
        map(tag("html"), |_| Ok(Extractor::Html)),
        map(tag("inner-html"), |_| Ok(Extractor::InnerHtml)),
        map(tag("hash"), |_| Ok(Extractor::Hash)),
        map(
            tuple((tag_whitespace("attr"), escaped_string)),
            |(_, attr)| Ok(Extractor::Attr(attr.to_owned())),
        ),
    ))(i)
}

#[test]
fn extractor_test() {
    // No `PartialEq` for me.
    match extractor("capture \n\t \"$(:!?foo)*\"").unwrap().1.unwrap() {
        Extractor::Capture(regex) => assert_eq!(
            Regex::from_str("$(:!?foo)*").unwrap().as_str(),
            regex.as_str()
        ),
        e => panic!("got {:?}", e),
    }
}

fn aggregator(i: &str) -> IResult<&str, Result<Aggregator, String>> {
    alt((
        map(tag("count"), |_| Ok(Aggregator::Count)),
        map(
            tuple((tag_whitespace("first"), extractor)),
            |(_, extractor)| Ok(Aggregator::First(extractor?)),
        ),
        map(
            tuple((tag_whitespace("collect"), extractor)),
            |(_, extractor)| Ok(Aggregator::Collect(extractor?)),
        ),
    ))(i)
}

#[test]
fn aggregator_test() {
    // No `PartialEq` for me.
    match aggregator("first capture \n\t \"$(:!?foo)*\"")
        .unwrap()
        .1
        .unwrap()
    {
        Aggregator::First(Extractor::Capture(regex)) => assert_eq!(
            Regex::from_str("$(:!?foo)*").unwrap().as_str(),
            regex.as_str()
        ),
        e => panic!("got {:?}", e),
    }
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

#[derive(Debug, Clone)]
pub struct RuleSet {
    pub in_page: Option<Regex>,
    pub selector: scraper::Selector,
    pub aggregators: HashMap<String, Aggregator>,
}

fn rule_set(i: &str) -> IResult<&str, Result<RuleSet, String>> {
    map(
        block(
            tuple((
                tag_whitespace("select"),
                opt(trailing_whitespace(in_directive)),
                |i| css_selector(i, '{'),
            )),
            identified_value(aggregator),
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
    rule_set("td > a[href^=\"https\"] { foo: first text ; }")
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

#[derive(Debug)]
#[non_exhaustive]
pub enum Item {
    Seed(Url),
    Boundary(Boundary),
    Module(Module),
    RuleSet(RuleSet),
}

fn item(i: &str) -> IResult<&str, Result<Item, String>> {
    alt((
        map(rule_set, |rule_set| Ok(Item::RuleSet(rule_set?))),
        map(module, |module| Ok(Item::Module(module))),
        map(seed, |seed| Ok(Item::Seed(seed?))),
        map(boundary, |boundary| Ok(Item::Boundary(boundary?))),
    ))(i)
}

#[test]
fn item_test() {
    item("td > a[href^=\"https\"] { foo: first text ; }")
        .unwrap()
        .1
        .unwrap();
}

pub fn entrypoint(i: &str) -> IResult<&str, Result<Vec<Item>, String>> {
    all_consuming(map(
        tuple((whitespace, separated_list(whitespace, item), whitespace)),
        |(_, results, _)| results.into_iter().collect::<Result<Vec<_>, _>>(),
    ))(i)
}
