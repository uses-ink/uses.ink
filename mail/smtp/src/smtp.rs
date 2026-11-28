//! The adapter.

use lettre::message::Mailbox;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use mail::{Address, Mailer, Message};
use secret::Plain;

use crate::Error;

/// How the connection to the server is secured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Security {
    /// Starts in the clear and upgrades with `STARTTLS`, which the server must offer (port 587).
    #[default]
    Starttls,
    /// TLS from the first byte (port 465).
    Implicit,
    /// No encryption at all, for a server on the same machine or a test. A password sent this
    /// way is readable on the wire.
    Plain,
}

/// A username and the password that goes with it.
#[derive(Debug)]
pub struct Login {
    /// The username.
    pub username: String,
    /// The password, read from the secret port by the caller.
    pub password: Plain,
}

/// Everything a connection needs.
#[derive(Debug)]
pub struct Settings {
    /// The server's host name.
    pub host: String,
    /// The server's port.
    pub port: u16,
    /// How the connection is secured.
    pub security: Security,
    /// What to log in with, if the server asks.
    pub login: Option<Login>,
    /// The address messages are sent from.
    pub from: Address,
}

/// The SMTP mailer.
pub struct Smtp {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl std::fmt::Debug for Smtp {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "Smtp(from {})", self.from)
    }
}

impl Smtp {
    /// A mailer for `settings`. No connection is made until the first send.
    pub fn new(settings: Settings) -> Result<Self, Error> {
        let builder = match settings.security {
            Security::Starttls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&settings.host)?,
            Security::Implicit => AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.host)?,
            Security::Plain => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&settings.host),
        };
        let builder = builder.port(settings.port);
        let builder = match settings.login {
            Some(Login { username, password }) => {
                builder.credentials(Credentials::new(username, String::from_utf8_lossy(password.expose()).into_owned()))
            }
            None => builder,
        };
        let from = Mailbox::new(None, settings.from.as_str().parse()?);
        Ok(Self { transport: builder.build(), from })
    }

    /// The MIME message for `message`, as it would be sent.
    pub fn compose(&self, message: &Message) -> Result<lettre::Message, Error> {
        let to = Mailbox::new(None, message.to.as_str().parse()?);
        Ok(lettre::Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(message.template.subject())
            .header(ContentType::TEXT_PLAIN)
            .body(message.template.to_string())?)
    }
}

impl Mailer for Smtp {
    type Error = Error;

    async fn send(&self, message: &Message) -> Result<(), Error> {
        self.transport.send(self.compose(message)?).await?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn settings() -> Settings {
        Settings {
            host: "mail.example".into(),
            port: 587,
            security: Security::Starttls,
            login: Some(Login { username: "ink".into(), password: Plain::from("hunter2") }),
            from: "ink@example.org".parse().unwrap(),
        }
    }

    #[test]
    fn composes_a_plain_text_message_from_the_template() {
        let smtp = Smtp::new(settings()).unwrap();
        let base: mail::LinkBase = "https://ink.example/login".parse().unwrap();
        let template = mail::Template::Login { link: base.link("abc"), valid: std::time::Duration::from_mins(15) };
        let composed = smtp.compose(&Message { to: "me@x.org".parse().unwrap(), template }).unwrap();
        let text = String::from_utf8(composed.formatted()).unwrap();
        assert!(text.contains("Subject: Your sign-in link"), "{text}");
        assert!(text.contains("To: me@x.org"), "{text}");
        assert!(text.contains("From: ink@example.org"), "{text}");
        assert!(!text.contains("hunter2"));
    }

    #[test]
    fn the_password_is_in_no_debug_output() {
        let text = format!("{:?}", settings());
        assert!(!text.contains("hunter2"), "{text}");
        assert!(text.contains("redacted"), "{text}");
        assert!(!format!("{:?}", Smtp::new(settings()).unwrap()).contains("hunter2"));
    }
}
