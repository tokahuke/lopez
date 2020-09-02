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

impl Reason {
    pub fn is_ahref(&self) -> bool {
        if let Reason::Ahref = self {
            true
        } else {
            false
        }
    }

    pub fn is_canonical(&self) -> bool {
        if let Reason::Canonical = self {
            true
        } else {
            false
        }
    }

    pub fn is_redirect(&self) -> bool {
        if let Reason::Redirect = self {
            true
        } else {
            false
        }
    }
}
