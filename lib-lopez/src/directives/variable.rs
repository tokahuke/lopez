use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

use super::value_ext::force_f64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Variable {
    UserAgent,
    Quota,
    MaxDepth,
    MaxHitsPerSec,
    RequestTimeout,
    MaxBodySize,
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Variable::UserAgent => "user_agent",
                Variable::Quota => "quota",
                Variable::MaxDepth => "max_depth",
                Variable::MaxHitsPerSec => "max_hits_per_sec",
                Variable::RequestTimeout => "request_timeout",
                Variable::MaxBodySize => "max_body_size",
            }
        )
    }
}

impl Variable {
    pub fn try_parse(input: &str) -> Option<Variable> {
        Some(match input {
            "user_agent" => Variable::UserAgent,
            "quota" => Variable::Quota,
            "max_depth" => Variable::MaxDepth,
            "max_hits_per_sec" => Variable::MaxHitsPerSec,
            "request_timeout" => Variable::RequestTimeout,
            "max_body_size" => Variable::MaxBodySize,
            _ => return None,
        })
    }

    fn bad_value<T>(&self, literal: &Value) -> Result<T, crate::Error> {
        Err(crate::Error::BadSetVariableValue(
            *self,
            literal.clone(),
        ))
    }

    fn retrieve_as_str<'a>(&self, literal: Option<&'a Value>) -> Result<&'a str, crate::Error> {
        match (self, literal) {
            (Variable::UserAgent, None) => Ok(crate::default_user_agent()),
            (Variable::UserAgent, Some(Value::String(user_agent))) => Ok(&*user_agent),
            (Variable::UserAgent, Some(literal)) => self.bad_value(literal),
            _ => panic!("cannot cast as string: {:?}", self),
        }
    }

    // TODO: when "or patterns" stabilize, refactor this code.

    fn retrieve_as_positive_f64(&self, literal: Option<&Value>) -> Result<f64, crate::Error> {
        match (self, literal) {
            (Variable::MaxHitsPerSec, None) => Ok(2.5),
            (Variable::RequestTimeout, None) => Ok(60.0),
            (Variable::MaxHitsPerSec, Some(Value::Number(number))) => {
                let number = force_f64(number);

                if number > 0. {
                    Ok(number)
                } else {
                    self.bad_value(&number.into())
                }
            }
            (Variable::RequestTimeout, Some(Value::Number(number))) => {
                let number = force_f64(number);

                if number > 0. {
                    Ok(number)
                } else {
                    self.bad_value(&number.into())
                }
            }
            (Variable::MaxHitsPerSec, Some(literal)) => self.bad_value(literal),
            (Variable::RequestTimeout, Some(literal)) => self.bad_value(literal),
            (_, _) => panic!("cannot cast as positive float: {:?}", self),
        }
    }

    fn retrieve_as_u64(&self, literal: Option<&Value>) -> Result<u64, crate::Error> {
        match (self, literal) {
            (Variable::Quota, None) => Ok(1000),
            (Variable::MaxDepth, None) => Ok(7),
            (Variable::MaxBodySize, None) => Ok(10_000_000),
            (Variable::Quota, Some(value)) => {
                if let Some(number) = value.as_u64() {
                    Ok(number)
                } else {
                    self.bad_value(value)
                }
            }
            (Variable::MaxDepth, Some(value)) => {
                if let Some(number) = value.as_u64() {
                    Ok(number)
                } else {
                    self.bad_value(value)
                }
            }
            (Variable::MaxBodySize, Some(value)) => {
                if let Some(number) = value.as_u64() {
                    Ok(number)
                } else {
                    self.bad_value(value)
                }
            }
            _ => panic!("cannot cast as usize: {:?}", self),
        }
    }
}

#[derive(Debug)]
pub struct SetVariables {
    pub(super) set_variables: BTreeMap<Variable, Value>,
}

impl SetVariables {
    pub fn get_as_str(&self, name: Variable) -> Result<&str, crate::Error> {
        name.retrieve_as_str(self.set_variables.get(&name))
    }

    pub fn get_as_positive_f64(&self, name: Variable) -> Result<f64, crate::Error> {
        name.retrieve_as_positive_f64(self.set_variables.get(&name))
    }

    pub fn get_as_u64(&self, name: Variable) -> Result<u64, crate::Error> {
        name.retrieve_as_u64(self.set_variables.get(&name))
    }
}
