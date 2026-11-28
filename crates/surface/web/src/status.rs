//! The status of a reply, as a closed set the edge maps to its HTTP library's.

use std::fmt;

/// What a reply says happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Status {
    /// The page is here.
    Ok,
    /// The change was made; go and look at the page named in `Location`.
    SeeOther,
    /// The request could not be read.
    BadRequest,
    /// Nobody is signed in.
    Unauthorized,
    /// The signed-in account may not see this.
    Forbidden,
    /// There is no such page, site or build.
    NotFound,
    /// The page does not take that method.
    MethodNotAllowed,
    /// The change collides with the state of the site.
    Conflict,
    /// Something of ours failed.
    Internal,
}

impl Status {
    /// The number HTTP gives it.
    pub const fn number(self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::SeeOther => 303,
            Self::BadRequest => 400,
            Self::Unauthorized => 401,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::MethodNotAllowed => 405,
            Self::Conflict => 409,
            Self::Internal => 500,
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.number())
    }
}
