use failure_derive::Fail;

#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display = "bad set-variable value for {}: {}", _0, _1)]
    BadSetVariableValue(super::Variable, serde_json::Value),
    #[fail(display = "type error: no type for `{}` of `{}`", _0, _1)]
    TypeError(String, crate::Type),
    #[fail(display = "{}", _0)]
    Custom(String),
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
