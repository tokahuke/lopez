use failure_derive::Fail;
use std::io;

#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display = "http error: {}", _0)]
    Http(hyper::Error),
    #[fail(display = "invalid uri: {}", _0)]
    InvalidUri(http::uri::InvalidUri),
    #[fail(display = "url parse error: {}", _0)]
    UrlParseError(url::ParseError),
    #[fail(display = "io: error: {}", _0)]
    Io(io::Error),
    #[fail(display = "no location header on redirect")]
    NoLocationOnRedirect,
    #[fail(display = "unknown Content-Encoding: {}", _0)]
    UnknownContentEncoding(String),
    #[fail(display = "timed out")]
    Timeout,
    #[fail(display = "{}", _0)]
    Custom(String),
}

impl From<hyper::Error> for Error {
    fn from(this: hyper::Error) -> Error {
        Error::Http(this)
    }
}

impl From<http::uri::InvalidUri> for Error {
    fn from(this: http::uri::InvalidUri) -> Error {
        Error::InvalidUri(this)
    }
}

impl From<url::ParseError> for Error {
    fn from(this: url::ParseError) -> Error {
        Error::UrlParseError(this)
    }
}

impl From<io::Error> for Error {
    fn from(this: io::Error) -> Error {
        Error::Io(this)
    }
}

impl From<String> for Error {
    fn from(this: String) -> Error {
        Error::Custom(this)
    }
}

// Is this Cow-style trick justified?
impl<'a> From<&'a Error> for Error {
    fn from(this: &'a Error) -> Error {
        Error::Custom(format!("{}", this))
    }
}
