//! Every page rendered from fixture view models and compared with its golden, a structural check
//! over all of them (one `h1`, every control labelled, every form named), and the dashboard's
//! routing, redirects and stream over a fake of what the edge provides.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]
#![allow(missing_docs, reason = "a test file")]

mod account;
mod builds;
mod deploys;
mod failure;
mod fixtures;
mod limits;
mod routing;
mod settings;
mod site;
mod structure;

use uses_ink_web::{Document, View};

/// The pages made so far, by name.
struct Pages(Vec<(String, String)>);

impl Pages {
    fn put(&mut self, name: &str, view: &dyn Pageable) {
        self.0.push((name.to_owned(), view.page()));
    }
}

/// What every screen's fixtures are made by.
struct Fixtures;

/// A view as its whole page.
trait Pageable {
    fn page(&self) -> String;
}

impl<V: View> Pageable for V {
    fn page(&self) -> String {
        Document(self).write().unwrap()
    }
}

#[test]
fn pages_match_their_goldens() {
    fixture::cases("fixture", Fixtures::pages());
}
