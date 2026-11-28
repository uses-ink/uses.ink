//! What the API's own routes said when they refused, kept as a failure of the dashboard.

use std::fmt;

use edge_api::{Reply, Status};
use serde::Deserialize;
use web::{Cause, Failure};

/// The problem document of a refusal: a stable code, a message and the causes under it.
#[derive(Debug, Deserialize)]
struct Problem {
    #[serde(rename = "problem")]
    code: String,
    message: String,
    #[serde(default)]
    causes: Vec<String>,
}

/// One link of what the API said: the message, and the cause under it.
#[derive(Debug)]
pub(crate) struct Said {
    message: String,
    below: Option<Box<Said>>,
}

impl fmt::Display for Said {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.message)
    }
}

impl std::error::Error for Said {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.below.as_deref().map(|below| below as &(dyn std::error::Error + 'static))
    }
}

/// A form body past the limit the server reads.
#[derive(Debug)]
pub(crate) struct Over(String);

impl fmt::Display for Over {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "the form body was not sent whole: {}", self.0)
    }
}

impl std::error::Error for Over {}

/// A request that came from somewhere that is not this dashboard.
#[derive(Debug)]
pub(crate) struct Elsewhere;

impl fmt::Display for Elsewhere {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the request did not come from this dashboard: its Origin is not one the server allows, and the browser did not say it was the same origin")
    }
}

impl std::error::Error for Elsewhere {}

impl Said {
    /// The API's status for the dashboard's.
    pub(crate) fn status_of(status: web::Status) -> Status {
        match status {
            web::Status::Ok => Status::Ok,
            web::Status::SeeOther => Status::SeeOther,
            web::Status::BadRequest => Status::BadRequest,
            web::Status::Unauthorized => Status::Unauthorized,
            web::Status::Forbidden => Status::Forbidden,
            web::Status::NotFound => Status::NotFound,
            web::Status::MethodNotAllowed => Status::MethodNotAllowed,
            web::Status::Conflict => Status::Conflict,
            web::Status::TooMany => Status::TooMany,
            web::Status::Unavailable => Status::Unavailable,
            _ => Status::Internal,
        }
    }

    /// A body that went over the limit, as an error to show.
    pub(crate) fn over(payload: &edge_api::Payload) -> Over {
        Over(format!("{payload:?}"))
    }

    /// What the server answered that cannot be read as the page needs.
    pub(crate) fn unread(source: &(dyn std::error::Error + 'static)) -> Failure {
        Failure::Auth {
            status: web::Status::Internal,
            problem: "reply.unreadable".to_owned(),
            source: Cause::of(source),
        }
    }

    /// The dashboard's status for the API's.
    pub(crate) fn status(status: Status) -> web::Status {
        match status {
            Status::Ok | Status::Created | Status::Accepted => web::Status::Ok,
            Status::SeeOther => web::Status::SeeOther,
            Status::BadRequest | Status::TooLarge | Status::Unprocessable => web::Status::BadRequest,
            Status::Unauthorized => web::Status::Unauthorized,
            Status::Forbidden => web::Status::Forbidden,
            Status::NotFound => web::Status::NotFound,
            Status::MethodNotAllowed => web::Status::MethodNotAllowed,
            Status::Conflict => web::Status::Conflict,
            Status::TooMany => web::Status::TooMany,
            Status::Unavailable => web::Status::Unavailable,
            _ => web::Status::Internal,
        }
    }

    /// The failure a refusing reply is: its status, its code and what it said, as a chain.
    pub(crate) fn failure(reply: &Reply) -> Failure {
        let status = Self::status(reply.status);
        match serde_json::from_slice::<Problem>(&reply.body) {
            Ok(problem) => {
                let below = problem
                    .causes
                    .into_iter()
                    .rev()
                    .fold(None, |below, message| Some(Box::new(Self { message, below })));
                let said = Self { message: problem.message, below };
                Failure::Auth { status, problem: problem.code, source: Cause::of(&said) }
            }
            Err(source) => Self::unread(&source),
        }
    }
}
