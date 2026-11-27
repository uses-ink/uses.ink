//! A mailer that writes each message to a writer.

use std::io::Write;
use std::sync::{Mutex, PoisonError};

use crate::{Mailer, Message};

/// Writes each message as text to `W`, which a development run points at its terminal: the link
/// is on screen and no mail server is needed. The writer is the caller's, so this crate prints
/// nothing of its own.
#[derive(Debug)]
pub struct Stdout<W>(Mutex<W>);

impl<W: Write> Stdout<W> {
    /// A mailer that writes to `writer`.
    pub const fn new(writer: W) -> Self {
        Self(Mutex::new(writer))
    }
}

impl<W: Write> Mailer for Stdout<W> {
    type Error = std::io::Error;

    fn send(&self, message: &Message) -> impl std::future::Future<Output = Result<(), std::io::Error>> {
        let mut writer = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let written = write!(writer, "mail to {}: {}\n{}\n", message.to, message.template.subject(), message.template)
            .and_then(|()| writer.flush());
        std::future::ready(written)
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use super::*;
    use crate::{Base, Template};

    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn writes_the_recipient_and_the_link() {
        let sink = Sink::default();
        let mailer = Stdout::new(sink.clone());
        let base: Base = "https://x.example/login".parse().unwrap();
        let template = Template::Login { link: base.link("tok"), valid: std::time::Duration::from_mins(10) };
        pollster::block_on(mailer.send(&Message { to: "me@x.org".parse().unwrap(), template })).unwrap();
        let text = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
        assert!(text.starts_with("mail to me@x.org: Your sign-in link\n"), "{text}");
        assert!(text.contains("https://x.example/login/tok"), "{text}");
    }
}
