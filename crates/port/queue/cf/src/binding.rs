//! The wake-up through a Cloudflare Queues binding.

use serde::Serialize;
use worker::MessageBuilder;

use crate::wake::{Nudge, Wake};

/// What a wake-up message says. A consumer reads the rows, not the message.
#[derive(Clone, Copy, Debug, Serialize)]
struct Body {
    wake: &'static str,
}

/// A Queues binding of a Worker, used only to wake the consumer.
#[derive(Debug)]
pub struct Queues {
    queue: worker::Queue,
}

impl Queues {
    /// The queue a Worker's binding gives, from `env.queue(..)`.
    pub const fn new(queue: worker::Queue) -> Self {
        Self { queue }
    }
}

impl Wake for Queues {
    type Error = worker::Error;

    async fn wake(&self, nudge: Nudge) -> Result<(), worker::Error> {
        let body = Body { wake: "queue" };
        self.queue.send(MessageBuilder::new(body).delay_seconds(nudge.seconds()).build()).await
    }
}
