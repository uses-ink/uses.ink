//! The limits page: every budget of the server, with its number and what happens at it. It is
//! public, and it is made from the same [`control::Limits`] the refusals read (decision 0088).

use std::fmt;

use control::Limits as Set;
use unspliced::{Halt, Render, Sink, html};

use crate::shell::{Frame, View};
use crate::style::look;

/// What the limits page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
    frame: Frame,
    limits: Set,
}

impl Limits {
    /// The page of `limits`.
    pub const fn new(limits: Set) -> Self {
        Self { frame: Frame { site: None, here: None }, limits }
    }
}

impl View for Limits {
    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn title(&self) -> impl fmt::Display + '_ {
        "Limits"
    }

    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let rows = self.limits.list();
        html! {
            p.look::LEDE { "What this server limits. A request over a limit is refused with the number it reached and the limit, and nothing of it is kept." }
            div.look::TABLE role="region" aria-label="Limits" tabindex=0 {
                table {
                    caption { "Limits" }
                    thead { tr { th scope="col" { "Limit" } th scope="col" { "Value" } th scope="col" { "At the limit" } } }
                    tbody {
                        @for (budget, amount) in &rows {
                            tr { th scope="row" { (budget.name()) } td { (amount.to_string()) } td { (budget.rule()) } }
                        }
                    }
                }
            }
        }
        .render(out)
    }
}
