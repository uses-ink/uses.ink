//! Tags paired among siblings. comrak hands raw HTML over tag by tag, so an element opened by
//! one sibling and closed by a later one is paired here. The same stack pairs inline elements
//! among inlines and `<details>` among blocks; a [`Frame`] says what each becomes.

use model::Span;

use super::Node;
use crate::error::{Error, Report};

/// An element opened by a tag and not yet closed, gathering the nodes after it.
pub(crate) trait Frame: Sized {
    /// What it holds and becomes.
    type Node: Node;

    /// The tag name that closes it.
    fn name(&self) -> &str;

    /// Where its opening tag is.
    fn span(&self) -> Span;

    /// Where nodes go while it is open.
    fn children(&mut self) -> &mut Vec<Self::Node>;

    /// The node it becomes, closed by its tag.
    fn close(self) -> Self::Node;

    /// What is kept in its place when it is never closed: its content.
    fn spill(self) -> Vec<Self::Node>;
}

/// How a closing tag paired.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Closing {
    /// It closed the innermost open element of its name.
    Paired,
    /// It closes an element whose opening was already reported as removed.
    Silenced,
    /// It closes nothing.
    Unmatched,
}

/// The nodes of one run of siblings, with the elements still open in it.
#[derive(Debug)]
pub(crate) struct Stack<F: Frame> {
    done: Vec<F::Node>,
    open: Vec<F>,
    /// Names of elements whose opening was reported as removed, so their closing tag says
    /// nothing more.
    refused: Vec<String>,
    /// Inside `<script>` or `<style>`, whose content is code, not prose: dropped until it closes.
    opaque: bool,
}

impl<F: Frame> Default for Stack<F> {
    fn default() -> Self {
        Self { done: Vec::new(), open: Vec::new(), refused: Vec::new(), opaque: false }
    }
}

impl<F: Frame> Stack<F> {
    /// Where the next node goes: into the innermost open element, or the run itself.
    fn target(&mut self) -> &mut Vec<F::Node> {
        match self.open.last_mut() {
            Some(open) => open.children(),
            None => &mut self.done,
        }
    }

    /// Adds a node, unless inside code.
    pub(crate) fn push(&mut self, node: F::Node) {
        if !self.opaque {
            F::Node::join(self.target(), node);
        }
    }

    /// Opens an element: the nodes after it are its children until it closes.
    pub(crate) fn open(&mut self, frame: F) {
        self.open.push(frame);
    }

    /// The innermost open element.
    pub(crate) fn top(&self) -> Option<&F> {
        self.open.last()
    }

    /// The innermost open element, to change.
    pub(crate) fn top_mut(&mut self) -> Option<&mut F> {
        self.open.last_mut()
    }

    /// Closes the innermost open element called `name`. Elements opened inside it and never
    /// closed are removed, each reported, their content kept.
    pub(crate) fn close(&mut self, name: &str, report: &mut Report) -> Closing {
        if let Some(at) = self.open.iter().rposition(|open| open.name() == name) {
            while self.open.len() > at + 1 {
                self.spill(report);
            }
            if let Some(open) = self.open.pop() {
                self.push(open.close());
            }
            Closing::Paired
        } else if let Some(at) = self.refused.iter().position(|refused| refused == name) {
            self.refused.remove(at);
            Closing::Silenced
        } else {
            Closing::Unmatched
        }
    }

    /// Notes that the opening tag of `name` was reported as removed, so its closing tag is
    /// silent.
    pub(crate) fn refuse(&mut self, name: impl Into<String>) {
        self.refused.push(name.into());
    }

    /// Enters or leaves code whose content is dropped.
    pub(crate) const fn hide(&mut self, opaque: bool) {
        self.opaque = opaque;
    }

    /// Whether what comes now is the code inside `<script>` or `<style>`.
    pub(crate) const fn is_opaque(&self) -> bool {
        self.opaque
    }

    /// Whether nothing that shows was gathered and nothing is open.
    pub(crate) fn is_blank(&self) -> bool {
        self.open.is_empty() && !F::Node::shows(&self.done)
    }

    /// The run, with every element still open removed and reported.
    pub(crate) fn finish(mut self, report: &mut Report) -> Vec<F::Node> {
        while !self.open.is_empty() {
            self.spill(report);
        }
        self.done
    }

    /// The innermost open element, never closed: reported, its content kept in its place.
    fn spill(&mut self, report: &mut Report) {
        let Some(open) = self.open.pop() else { return };
        report.refuse(Error::Stripped { element: open.name().to_owned() }, open.span());
        self.refused.push(open.name().to_owned());
        for node in open.spill() {
            self.push(node);
        }
    }
}
