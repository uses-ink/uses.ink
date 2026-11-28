//! Recovery codes on the Account screen: how many are unspent, and a button that makes a new set.
//! Making them is a passkey step-up and a JSON call, so it runs in the one script (`/passkey.js`),
//! which writes the codes into the page once; they are never in the server-rendered page.

use unspliced::keyword::ButtonType;
use unspliced::{Halt, Render, Sink, html};

use crate::style::look;

/// The recovery codes section of the Account screen: `left` codes are unspent.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Section(pub(crate) u64);

impl Render for Section {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        html! {
            h2 { "Recovery codes" }
            p { "If you lose every passkey, a recovery code lets you register a new one. Each works once." }
            @if self.0 == 0 {
                p { "You have no unspent recovery codes." }
            } @else {
                p { "You have " (self.0) " unspent recovery codes." }
            }
            div.look::STACK {
                button type=(ButtonType::Button) data-passkey="mint" { "Make new recovery codes" }
                p { "A new set ends the old one. The codes are shown once, here, and kept nowhere: write them down or put them in a password manager." }
                pre data-recovery-codes="" hidden {}
            }
        }
        .render(out)
    }
}
