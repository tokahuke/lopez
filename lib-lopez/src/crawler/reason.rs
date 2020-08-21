#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Ahref,
    Redirect,
    Canonical,
}

impl ToString for Reason {
    fn to_string(&self) -> String {
        match self {
            Reason::Ahref => "ahref",
            Reason::Redirect => "redirect",
            Reason::Canonical => "canonical",
        }
        .to_owned()
    }
}
