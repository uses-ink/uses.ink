//! A failure of a port whose error type the caller does not name.

use std::error::Error;
use std::fmt;

/// Which port failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Port {
    /// The [`crate::Credentials`] port could not check a credential.
    Credentials,
    /// The [`clock::Draw`] port gave no bytes.
    Draw,
    /// The mailer could not send a message.
    Mailer,
}

/// The error of a port, kept as the cause of a failure that names the port. The port's own error
/// type is a parameter of the code that calls it, not of the values that report it, so the cause
/// is held by what it can do: say what happened, and say why.
#[derive(Debug)]
pub struct Unnamed {
    port: Port,
    cause: Box<dyn Error>,
}

impl Unnamed {
    /// The failure of `port`, from `cause`.
    pub fn new<E: Error + 'static>(port: Port, cause: E) -> Self {
        Self { port, cause: Box::new(cause) }
    }

    /// Which port failed.
    pub const fn port(&self) -> Port {
        self.port
    }

    /// The port's own error.
    pub fn cause(&self) -> &(dyn Error + 'static) {
        self.cause.as_ref()
    }
}

impl fmt::Display for Unnamed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self.port {
            Port::Credentials => "the credential could not be checked",
            Port::Draw => "no random bytes were available",
            Port::Mailer => "a sign-in message could not be sent",
        })
    }
}

impl Error for Unnamed {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

impl AsRef<dyn Error> for Unnamed {
    fn as_ref(&self) -> &(dyn Error + 'static) {
        self.cause.as_ref()
    }
}
