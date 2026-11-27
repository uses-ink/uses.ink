//! The writer: a [`Doc`] becomes bytes, painted through the theme or left plain.

use std::io::{self, Write};

use anstream::adapter::strip_str;

use super::doc::{Doc, Line, Piece, Render};
use super::look::Look;

/// A stream and how its output looks.
#[derive(Debug)]
pub(crate) struct Writer<W> {
    sink: W,
    look: Look,
}

impl<W: Write> Writer<W> {
    /// Carriage return, then erase the line: how a live line is taken back.
    const ERASE: &'static str = "\r\x1b[2K";

    /// A writer onto `sink`.
    pub(crate) const fn new(sink: W, look: Look) -> Self {
        Self { sink, look }
    }

    /// How this stream looks.
    pub(crate) const fn look(&self) -> Look {
        self.look
    }

    /// The stream itself, for the JSON that is written as it is.
    pub(crate) fn sink(&mut self) -> &mut W {
        &mut self.sink
    }

    /// Renders `block` and writes its lines, each ended by a newline.
    pub(crate) fn emit(&mut self, block: &(impl Render + ?Sized)) -> io::Result<()> {
        let mut doc = Doc::new(self.look);
        block.render(&mut doc);
        for line in doc.lines() {
            for piece in line.pieces() {
                self.piece(piece)?;
            }
            writeln!(self.sink)?;
        }
        Ok(())
    }

    /// Replaces the line the cursor is on with the first line of `block`, leaving the cursor at
    /// its end: how a live line is redrawn.
    pub(crate) fn redraw(&mut self, block: &(impl Render + ?Sized)) -> io::Result<()> {
        let mut doc = Doc::new(self.look);
        block.render(&mut doc);
        self.erase()?;
        for piece in doc.lines().first().into_iter().flat_map(Line::pieces) {
            self.piece(piece)?;
        }
        self.sink.flush()
    }

    /// Takes back the live line.
    pub(crate) fn erase(&mut self) -> io::Result<()> {
        self.sink.write_all(Self::ERASE.as_bytes())
    }

    fn piece(&mut self, piece: &Piece) -> io::Result<()> {
        if piece.text.is_empty() {
            return Ok(());
        }
        match (self.look.colour, piece.styled) {
            (false, true) => write!(self.sink, "{}", strip_str(&piece.text)),
            (true, false) => {
                let style = self.look.theme().style(piece.role, piece.strength);
                write!(self.sink, "{style}{}{style:#}", piece.text)
            }
            (true, true) | (false, false) => write!(self.sink, "{}", piece.text),
        }
    }
}
