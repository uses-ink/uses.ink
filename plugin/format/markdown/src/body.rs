//! The body: comrak's tree built into a [`Document`]. One [`Builder`] walks the tree; a run of
//! siblings is read once, generically, by [`Builder::run`], and each [`Node`] kind (blocks,
//! inlines) says how one comrak child joins its run. Raw HTML among the siblings is paired on
//! the run's [`Stack`].

mod block;
mod element;
mod html;
mod inline;
mod name;
mod position;
mod stack;

use std::fmt;

use comrak::nodes::{AstNode, NodeValue};
use model::{Document, Footnote, Spanned};

pub use inline::Plain;

use crate::dialect::{Callouts, Dialect, Extension};
use crate::error::{DEPTH, Error, Report};
use crate::file::File;
use name::Names;
use position::Positions;
use stack::{Frame, Stack};

/// The body read: the document and its first level-1 heading.
#[derive(Debug, Default)]
pub(crate) struct Body {
    pub(crate) document: Document,
    /// The text of the first level-1 heading and where it is.
    pub(crate) heading: Option<Spanned<String>>,
}

/// A kind of model node a run of comrak siblings becomes: [`model::Block`] or
/// [`model::Inline`].
pub(crate) trait Node: Sized {
    /// An element raw HTML opened in a run of these and has not closed yet.
    type Open: Frame<Node = Self>;

    /// Reads the comrak node `child` into `run`.
    fn read<'a, D: Dialect>(child: &'a AstNode<'a>, builder: &mut Builder<'_, D>, run: &mut Stack<Self::Open>);

    /// What a run nested too deep keeps of the children of `parent`: their text.
    fn flatten<'a>(parent: &'a AstNode<'a>) -> Option<Self>;

    /// Adds `node` at the end of `run`.
    fn join(run: &mut Vec<Self>, node: Self);

    /// Whether `run` shows anything to a reader.
    fn shows(run: &[Self]) -> bool;
}

/// A comrak value with a model counterpart it becomes as is, needing nothing from the walk.
pub(crate) trait ToModel {
    /// What it becomes.
    type Model;

    /// The model value.
    fn to_model(&self) -> Self::Model;
}

/// State while building one document from comrak's tree, in dialect `D`.
#[derive(Debug)]
pub(crate) struct Builder<'a, D> {
    positions: Positions<'a>,
    names: Names,
    report: &'a mut Report,
    dialect: &'a mut D,
    heading: Option<Spanned<String>>,
    /// Footnote definitions, in the order comrak gives them: by first reference.
    footnotes: Vec<Footnote>,
    depth: usize,
    /// How many links the walk is inside: a link's text is not the body's own text, so the
    /// dialect does not read meaning out of it.
    linked: usize,
}

impl<'a, D: Dialect> Builder<'a, D> {
    /// Reads the body of `file` in `dialect`, every problem into `report`.
    pub(crate) fn read(file: &File<'a>, report: &'a mut Report, dialect: &'a mut D) -> Body {
        let (Some(text), Some(positions)) = (file.text.get(file.body..), Positions::new(file.text, file.body)) else {
            return Body::default();
        };
        let arena = comrak::Arena::new();
        let root = comrak::parse_document(&arena, text, &Self::options());
        let names = Names::default();
        let mut builder =
            Self { positions, names, report, dialect, heading: None, footnotes: Vec::new(), depth: 0, linked: 0 };
        let blocks = builder.run(root);
        Body { document: Document::new(blocks).with_footnotes(builder.footnotes), heading: builder.heading }
    }

    /// The syntax the dialect reads: `CommonMark`, the extensions every dialect reads, and the
    /// ones its [`crate::dialect::Syntax`] turns on.
    fn options() -> comrak::Options<'static> {
        let syntax = D::SYNTAX;
        let mut options = comrak::Options::default();
        let extension = &mut options.extension;
        extension.strikethrough = true;
        extension.table = true;
        extension.autolink = true;
        extension.alerts = syntax.callouts == Callouts::Alerts;
        extension.ofm_callouts = syntax.callouts == Callouts::Folding;
        extension.math_dollars = true;
        extension.math_dollars_block = true;
        extension.math_code = true;
        extension.footnotes = true;
        extension.tasklist = true;
        extension.highlight = true;
        extension.superscript = syntax.has(Extension::Scripts);
        // `~x~` is subscript, so strikethrough is `~~x~~` only
        extension.subscript = syntax.has(Extension::Scripts);
        extension.description_lists = syntax.has(Extension::Definitions);
        extension.wikilinks_title_after_pipe = syntax.has(Extension::Wikilinks);
        extension.ofm_block_ids = syntax.has(Extension::BlockIds);
        extension.ofm_comments = syntax.has(Extension::Comments);
        options
    }

    /// The children of `parent` as a run of `N`, raw HTML among them paired. Past [`DEPTH`],
    /// only their text is kept, and that is reported.
    fn run<'n, N: Node>(&mut self, parent: &'n AstNode<'n>) -> Vec<N> {
        if self.depth >= DEPTH {
            let kept = N::flatten(parent).into_iter().collect();
            let span = self.positions.span(parent.data().sourcepos);
            self.report.refuse(Error::Nested, span);
            return kept;
        }
        self.depth += 1;
        let mut run = Stack::default();
        for child in parent.children() {
            N::read(child, self, &mut run);
        }
        self.depth -= 1;
        run.finish(self.report)
    }
}

/// The text of a comrak node and everything under it, without recursion: what a node nested
/// too deep keeps.
#[derive(Clone, Copy)]
pub(crate) struct Literal<'a>(pub(crate) &'a AstNode<'a>);

impl fmt::Display for Literal<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for node in self.0.descendants() {
            match &node.data().value {
                NodeValue::Text(text) => out.write_str(text)?,
                NodeValue::Code(code) => out.write_str(&code.literal)?,
                NodeValue::CodeBlock(code) => out.write_str(&code.literal)?,
                NodeValue::Math(math) => out.write_str(&math.literal)?,
                NodeValue::SoftBreak | NodeValue::LineBreak => out.write_str(" ")?,
                _ => {}
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Literal<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_tuple("Literal").field(&self.0.data().value).finish()
    }
}
