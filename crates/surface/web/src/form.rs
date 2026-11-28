//! A request as the dashboard reads it: a method, a path, and the fields of a query or a form
//! body. Forms are plain `application/x-www-form-urlencoded` posts answered by a redirect.

use std::fmt;

use build::BuildId;
use control::{AccountKind, DomainName, Handle, Id, OrgName, Role};
use model::{AnyCode, ContentPath, DeployId, Subject};

use crate::Failure;
use crate::failure::{FieldError, Unrouted};

/// The methods a dashboard page answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Method {
    /// Reading a page.
    Get,
    /// Sending a form.
    Post,
}

impl fmt::Display for Method {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Get => "GET",
            Self::Post => "POST",
        })
    }
}

/// A form field or query parameter the dashboard reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Field {
    /// The deploy to roll back to.
    Deploy,
    /// The handle typed to confirm a removal.
    Confirm,
    /// A filter: `check` shows only the decisions to check.
    Only,
    /// A decision code.
    Code,
    /// The file a decision is about; absent for the site.
    On,
    /// The email address a sign-in link is asked for, or given to the first-run link.
    Address,
    /// The custom domain a form is about, or the name of an organization to make.
    Name,
    /// The organization a form is about, by its account id.
    Org,
    /// The person a form is about, by their account id.
    Account,
    /// The role a member is given.
    Role,
}

impl Field {
    /// The name the form carries it under.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Deploy => "deploy",
            Self::Confirm => "confirm",
            Self::Only => "only",
            Self::Code => "code",
            Self::On => "on",
            Self::Address => "address",
            Self::Name => "name",
            Self::Org => "org",
            Self::Account => "account",
            Self::Role => "role",
        }
    }
}

impl fmt::Display for Field {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// The decoded fields of a query string or a form body, in the order sent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Form(Vec<(String, String)>);

impl Form {
    /// The first value sent for `field`.
    pub fn value(&self, field: Field) -> Option<&str> {
        self.0.iter().find(|(name, _)| name == field.name()).map(|(_, value)| value.as_str())
    }

    /// The value of `field`, or the failure that says the form lacks it.
    pub fn need(&self, field: Field) -> Result<&str, Failure> {
        self.value(field).ok_or(Failure::Missing { field })
    }
}

impl From<&str> for Form {
    /// Reads `a=1&b=two%20words`: `+` is a space, `%xx` a byte, and bytes that are not UTF-8
    /// come out as the replacement character (they match no field value we accept).
    fn from(text: &str) -> Self {
        Self(
            text.split('&')
                .filter(|pair| !pair.is_empty())
                .map(|pair| {
                    let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                    (Percent(name).decode(), Percent(value).decode())
                })
                .collect(),
        )
    }
}

/// Text in a form's percent encoding.
#[derive(Clone, Copy, Debug)]
struct Percent<'a>(&'a str);

impl Percent<'_> {
    fn digit(byte: u8) -> Option<u8> {
        char::from(byte).to_digit(16).and_then(|digit| u8::try_from(digit).ok())
    }

    fn decode(self) -> String {
        let bytes = self.0.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut at = 0;
        while let Some(&byte) = bytes.get(at) {
            at += 1;
            match byte {
                b'+' => out.push(b' '),
                b'%' => {
                    match (bytes.get(at).and_then(|b| Self::digit(*b)), bytes.get(at + 1).and_then(|b| Self::digit(*b)))
                    {
                        (Some(high), Some(low)) => {
                            out.push(high * 16 + low);
                            at += 2;
                        }
                        _ => out.push(b'%'),
                    }
                }
                other => out.push(other),
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }
}

/// A request: what the edge hands the dashboard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request<'a> {
    /// The method.
    pub method: Method,
    /// The path, without its query.
    pub path: &'a str,
    /// The query string's fields.
    pub query: Form,
    /// The form body's fields, for a post.
    pub body: Form,
}

/// A change a form asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
    /// Build the site now.
    Build,
    /// Serve an older deploy, and pause publishing.
    Rollback {
        /// The deploy to go back to.
        deploy: DeployId,
    },
    /// Publish again after a rollback.
    Resume,
    /// Remove the site, which the person confirmed by typing its handle.
    Remove {
        /// What they typed.
        confirmed: Handle,
    },
    /// Ask to prove a custom domain: the answer is the record to publish.
    Claim {
        /// The name.
        name: DomainName,
    },
    /// Read the domain's record and, when it proves the claim, serve the site at the name.
    Prove {
        /// The name.
        name: DomainName,
    },
    /// Stop serving a custom domain, or drop its claim.
    Unclaim {
        /// The name.
        name: DomainName,
    },
}

impl Action {
    /// The action a post to `name` asks for, read from `body`.
    pub fn read(name: &str, body: &Form) -> Result<Self, Failure> {
        match name {
            "build" => Ok(Self::Build),
            "resume" => Ok(Self::Resume),
            "rollback" => {
                let deploy = body.need(Field::Deploy)?;
                let deploy = deploy
                    .parse()
                    .map_err(|source| Failure::Invalid { field: Field::Deploy, source: FieldError::Deploy(source) })?;
                Ok(Self::Rollback { deploy })
            }
            "remove" => {
                let typed = body.need(Field::Confirm)?;
                let confirmed = typed
                    .parse()
                    .map_err(|source| Failure::Invalid { field: Field::Confirm, source: FieldError::Handle(source) })?;
                Ok(Self::Remove { confirmed })
            }
            "claim" => Ok(Self::Claim { name: Self::domain(body)? }),
            "prove" => Ok(Self::Prove { name: Self::domain(body)? }),
            "unclaim" => Ok(Self::Unclaim { name: Self::domain(body)? }),
            _ => Err(Failure::Unrouted { source: Unrouted::Path }),
        }
    }

    /// The domain a form names.
    fn domain(body: &Form) -> Result<DomainName, Failure> {
        let typed = body.need(Field::Name)?;
        typed
            .trim()
            .parse()
            .map_err(|source| Failure::Invalid { field: Field::Name, source: FieldError::Domain(source) })
    }
}

/// A change to an organization or its members that a form on the Account screen asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seating {
    /// Make an organization, with the person as its owner.
    Create {
        /// What it is called.
        name: OrgName,
    },
    /// Add a person to an organization.
    Add {
        /// The organization.
        org: Id<AccountKind>,
        /// The person.
        account: Id<AccountKind>,
        /// What they may do there.
        role: Role,
    },
    /// Give a member another role.
    Regrade {
        /// The organization.
        org: Id<AccountKind>,
        /// The member.
        account: Id<AccountKind>,
        /// The role they get.
        role: Role,
    },
    /// Remove a member.
    Remove {
        /// The organization.
        org: Id<AccountKind>,
        /// The member.
        account: Id<AccountKind>,
    },
}

impl Seating {
    /// The change a post to `/account/<name>` asks for, read from `body`.
    pub fn read(name: &str, body: &Form) -> Result<Self, Failure> {
        let id = |field| -> Result<Id<AccountKind>, Failure> {
            body.need(field)?
                .trim()
                .parse()
                .map_err(|source| Failure::Invalid { field, source: FieldError::Account(source) })
        };
        let role = || -> Result<Role, Failure> {
            body.need(Field::Role)?
                .trim()
                .parse()
                .map_err(|source| Failure::Invalid { field: Field::Role, source: FieldError::Role(source) })
        };
        match name {
            "orgs" => {
                let name = body
                    .need(Field::Name)?
                    .parse()
                    .map_err(|source| Failure::Invalid { field: Field::Name, source: FieldError::Org(source) })?;
                Ok(Self::Create { name })
            }
            "members" => Ok(Self::Add { org: id(Field::Org)?, account: id(Field::Account)?, role: role()? }),
            "roles" => Ok(Self::Regrade { org: id(Field::Org)?, account: id(Field::Account)?, role: role()? }),
            "removals" => Ok(Self::Remove { org: id(Field::Org)?, account: id(Field::Account)? }),
            _ => Err(Failure::Unrouted { source: Unrouted::Path }),
        }
    }
}

/// A decision the "why" screen replays: its code and what it is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asked {
    /// The code.
    pub code: AnyCode,
    /// The subject.
    pub subject: Subject,
}

impl Asked {
    /// Reads `code` and `on` from a query.
    pub fn read(query: &Form) -> Result<Self, Failure> {
        let code =
            query.need(Field::Code)?.parse().map_err(|source| Failure::Unrouted { source: Unrouted::Code(source) })?;
        let subject = match query.value(Field::On) {
            None => Subject::Site,
            Some(path) => Subject::File(
                ContentPath::parse(path).map_err(|source| Failure::Unrouted { source: Unrouted::File(source) })?,
            ),
        };
        Ok(Self { code, subject })
    }
}

/// The secret segment of a sign-in link's path. The dashboard checks only its shape, so that it
/// can be written back into a form's address; the sign-in routes decide whether it is a link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// The most characters a token has: an address that long is no link.
    pub const LONGEST: usize = 128;

    /// The token `text` is, when it has the shape of one: 1 to [`Self::LONGEST`] characters of
    /// letters, digits, `-` and `_`.
    pub fn parse(text: &str) -> Option<Self> {
        let shaped = !text.is_empty()
            && text.len() <= Self::LONGEST
            && text.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        shaped.then(|| Self(text.to_owned()))
    }

    /// As text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Token {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// A build in a path.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Segment<'a>(pub(crate) &'a str);

impl Segment<'_> {
    /// The segment as a handle.
    pub(crate) fn handle(self) -> Result<Handle, Failure> {
        self.0.parse().map_err(|source| Failure::Unrouted { source: Unrouted::Handle(source) })
    }

    /// The segment as a link's token.
    pub(crate) fn token(self) -> Result<Token, Failure> {
        Token::parse(self.0).ok_or(Failure::Unrouted { source: Unrouted::Path })
    }

    /// The segment as a session id.
    pub(crate) fn session(self) -> Result<control::Id<control::SessionKind>, Failure> {
        self.0.parse().map_err(|source| Failure::Unrouted { source: Unrouted::Session(source) })
    }

    /// The segment as a build id.
    pub(crate) fn build(self) -> Result<BuildId, Failure> {
        self.0.parse().map_err(|source| Failure::Unrouted { source: Unrouted::Build(source) })
    }
}
