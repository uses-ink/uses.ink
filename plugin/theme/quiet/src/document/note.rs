//! Footnotes: a reference is the footnote's number, linking to it; the footnote links back to
//! every reference written before it.

use engine::Record;
use model::highlight::Highlight;
use model::{Footnote, FootnoteName};
use unspliced::{HERE, Halt, Id, Lazy, Render, Sink, Url, html};

use super::Writer;
use crate::style::quiet;

/// The ids of a footnote and of its references.
#[derive(Clone, Copy, Debug)]
struct Note<'a>(&'a FootnoteName);

impl Note<'_> {
    /// `fn:<name>`. A heading anchor is a slug, which never holds `:`, so the two never meet.
    fn id(self) -> String {
        format!("fn:{}", self.0)
    }

    /// `ref:<name>` for the first reference, `ref:<name>:<n>` for the n-th.
    fn reference(self, nth: usize) -> String {
        if nth == 1 { format!("ref:{}", self.0) } else { format!("ref:{}:{nth}", self.0) }
    }
}

impl<H: Highlight, R: Record + ?Sized> Writer<'_, H, R> {
    /// A reference to `name`: its number, linking to the footnote.
    pub(super) fn reference(&mut self, name: &FootnoteName) {
        let Some(&number) = self.numbers.get(name) else { return self.put(name.to_string()) };
        let count = self.references.entry(name.clone()).or_insert(0);
        *count += 1;
        let (note, reference) = (Note(name).id(), Note(name).reference(*count));
        match (Id::checked(&note), Id::checked(&reference)) {
            (Ok(note), Ok(reference)) => self.put(html! { sup { a id=(reference) href=(Url::to(note)) { (number) } } }),
            _ => self.put(html! { sup { (number) } }),
        }
    }

    /// One footnote as a list item, with links back to its references.
    pub(super) fn note(&mut self, footnote: &Footnote) {
        let id = Note(&footnote.name).id();
        let count = self.references.get(&footnote.name).copied().unwrap_or(0);
        let backs: Vec<String> = (1..=count).map(|nth| Note(&footnote.name).reference(nth)).collect();
        let back = Lazy(|out: &mut dyn Sink| {
            backs.iter().try_for_each(|back| match Id::checked(back) {
                Ok(back) => html! { a.quiet::BACK href=(Url::to(back)) aria-label="Back to the text" { "\u{21a9}" } }
                    .render(out),
                Err(_) => Ok::<(), Halt>(()),
            })
        });
        match Id::checked(&id) {
            Ok(id) => self.wrap(html! { li id=(id) { (HERE) (back) } }, |li| footnote.walk(li)),
            Err(_) => self.wrap(html! { li { (HERE) } }, |li| footnote.walk(li)),
        }
    }
}
