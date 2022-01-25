use nom::{
    branch::alt,
    bytes::complete::{escaped, is_not, tag},
    character::complete::{anychar, multispace1},
    combinator::map,
    multi::many0,
    sequence::{delimited, tuple},
    IResult,
};
use regex::Regex;
use std::str::FromStr;

/// Defines end of file (lol!):
pub fn eof(i: &str) -> IResult<&str, ()> {
    if i.is_empty() {
        Ok((i, ()))
    } else {
        Err(nom::Err::Error(nom::error::Error { input: i, code: nom::error::ErrorKind::IsA }))
    }
}

/// Defines a comment line. This is the only kind of comment.
pub fn comment(i: &str) -> IResult<&str, ()> {
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
pub fn whitespace(i: &str) -> IResult<&str, ()> {
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

pub fn trailing_whitespace<'a, F, T>(f: F) -> impl FnMut(&'a str) -> IResult<&'a str, T>
where
    F: FnMut(&'a str) -> IResult<&'a str, T>,
{
    map(tuple((f, whitespace)), |(t, _)| t)
}

#[allow(clippy::needless_lifetimes)] // not that needless...
pub fn tag_whitespace<'a>(tag_val: &'a str) -> impl FnMut(&'a str) -> IResult<&'a str, &'a str> {
    trailing_whitespace(tag(tag_val))
}

pub fn tags_whitespace<'a>(tag_vals: &'a [&'a str]) -> impl Fn(&'a str) -> IResult<&'a str, ()> {
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

pub fn escaped_string(i: &str) -> IResult<&str, String> {
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

pub fn regex(parsed: &str) -> Result<Regex, String> {
    Regex::from_str(parsed).map_err(|err| format!("{}", err))
}
