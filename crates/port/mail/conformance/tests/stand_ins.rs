//! The two stand-ins of the mail port against its suite: the capture, which keeps typed messages,
//! and the stdout mailer, which writes text and can fail when its writer does.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_mail_conformance as mail_conformance;

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use mail::{Capture, Stdout};
use mail_conformance::{Delivered, Harness, Refusing};

/// The capture, read back as what each message says.
struct Captured(Capture);

impl Harness for Captured {
    type Mailer = Capture;

    fn mailer(&self) -> &Capture {
        &self.0
    }

    fn delivered(&self) -> Vec<Delivered> {
        self.0
            .sent()
            .into_iter()
            .map(|message| Delivered {
                to: message.to.to_string(),
                subject: message.template.subject().to_owned(),
                body: message.template.to_string(),
            })
            .collect()
    }

    fn refuse(&self, _: bool) -> Refusing {
        Refusing::Impossible
    }
}

mail_conformance::suite!(|| (Captured(Capture::new()), ()));

/// A writer that keeps what it is given and can be told to fail.
#[derive(Clone, Default)]
struct Sink {
    bytes: Arc<Mutex<Vec<u8>>>,
    failing: Arc<AtomicBool>,
}

impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.failing.load(Ordering::SeqCst) {
            return Err(io::Error::other("the terminal is gone"));
        }
        self.bytes.lock().unwrap_or_else(PoisonError::into_inner).extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The stdout mailer, read back by splitting what it wrote at each `mail to` line.
struct Written(Stdout<Sink>, Sink);

impl Harness for Written {
    type Mailer = Stdout<Sink>;

    fn mailer(&self) -> &Stdout<Sink> {
        &self.0
    }

    fn delivered(&self) -> Vec<Delivered> {
        let text = String::from_utf8(self.1.bytes.lock().unwrap_or_else(PoisonError::into_inner).clone()).unwrap();
        let mut found: Vec<Delivered> = Vec::new();
        for line in text.lines() {
            match line.strip_prefix("mail to ").and_then(|rest| rest.split_once(": ")) {
                Some((to, subject)) => {
                    found.push(Delivered { to: to.to_owned(), subject: subject.to_owned(), body: String::new() });
                }
                None => {
                    if let Some(last) = found.last_mut() {
                        last.body.push_str(line);
                        last.body.push('\n');
                    }
                }
            }
        }
        found
    }

    fn refuse(&self, refusing: bool) -> Refusing {
        self.1.failing.store(refusing, Ordering::SeqCst);
        Refusing::Told
    }
}

mod stdout {
    use super::*;

    mail_conformance::suite!(|| {
        let sink = Sink::default();
        (Written(Stdout::new(sink.clone()), sink), ())
    });
}
