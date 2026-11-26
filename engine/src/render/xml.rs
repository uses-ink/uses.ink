//! The XML the engine serves, written through `quick-xml`: the one module that knows it. The
//! feeds and the sitemap write through [`Xml`] and see none of `quick-xml`'s types.

use std::{fmt, io};

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};

/// An XML document being written into memory.
pub(crate) struct Xml(Writer<Vec<u8>>);

impl fmt::Debug for Xml {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_tuple("Xml").field(&self.0.get_ref().len()).finish()
    }
}

impl Xml {
    /// A document started with its declaration, UTF-8.
    pub(crate) fn declared() -> io::Result<Self> {
        let mut out = Writer::new(Vec::new());
        out.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;
        Ok(Self(out))
    }

    /// `<name attributes..>`, what `inner` writes, then `</name>`.
    pub(crate) fn parent<'a>(
        &mut self,
        name: &str,
        attributes: impl IntoIterator<Item = (&'a str, &'a str)>,
        inner: impl FnOnce(&mut Self) -> io::Result<()>,
    ) -> io::Result<()> {
        self.0.write_event(Event::Start(BytesStart::new(name).with_attributes(attributes)))?;
        inner(self)?;
        self.0.write_event(Event::End(BytesEnd::new(name)))
    }

    /// `<name attributes..>text</name>`, the text escaped.
    pub(crate) fn text<'a>(
        &mut self,
        name: &str,
        attributes: impl IntoIterator<Item = (&'a str, &'a str)>,
        text: &str,
    ) -> io::Result<()> {
        self.parent(name, attributes, |xml| xml.0.write_event(Event::Text(BytesText::new(text))))
    }

    /// `<name>text</name>`.
    pub(crate) fn leaf(&mut self, name: &str, text: &str) -> io::Result<()> {
        self.text(name, [], text)
    }

    /// `<name attributes../>`.
    pub(crate) fn empty<'a>(
        &mut self,
        name: &str,
        attributes: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> io::Result<()> {
        self.0.write_event(Event::Empty(BytesStart::new(name).with_attributes(attributes)))
    }
}

impl From<Xml> for Vec<u8> {
    fn from(xml: Xml) -> Self {
        xml.0.into_inner()
    }
}
