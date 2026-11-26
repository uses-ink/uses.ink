//! math-core's output read back through an allowlist before it reaches a page: only `MathML`
//! elements and presentation attributes, text whose every `&` starts a reference, every element
//! closed in order. math-core escapes what it writes; this holds if it ever does not.

use std::fmt;

/// `MathML` read through the allowlist, to be written as is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Mathml(String);

impl Mathml {
    /// `MathML` presentation elements a page may hold. `annotation-xml` is not one: it can hold
    /// HTML.
    const ELEMENTS: &[&str] = &[
        "math",
        "mrow",
        "mi",
        "mn",
        "mo",
        "ms",
        "mtext",
        "mspace",
        "msub",
        "msup",
        "msubsup",
        "munder",
        "mover",
        "munderover",
        "mmultiscripts",
        "mprescripts",
        "none",
        "mfrac",
        "msqrt",
        "mroot",
        "mstyle",
        "merror",
        "mpadded",
        "mphantom",
        "mtable",
        "mtr",
        "mtd",
        "menclose",
        "semantics",
        "annotation",
    ];

    /// Elements math-core wraps text in (a link, a font span): left out, their text kept.
    const UNWRAPPED: &[&str] = &["a", "span"];

    /// Presentation attributes a page may hold.
    const ATTRIBUTES: &[&str] = &[
        "display",
        "displaystyle",
        "scriptlevel",
        "mathvariant",
        "mathsize",
        "stretchy",
        "symmetric",
        "largeop",
        "movablelimits",
        "fence",
        "separator",
        "form",
        "lspace",
        "rspace",
        "minsize",
        "maxsize",
        "accent",
        "accentunder",
        "linethickness",
        "width",
        "height",
        "depth",
        "voffset",
        "columnalign",
        "rowalign",
        "columnspan",
        "rowspan",
        "columnlines",
        "rowlines",
        "notation",
        "encoding",
        "dir",
    ];

    /// Attributes math-core decorates with that a reader page does without: `style`, which its
    /// CSP refuses anyway, and the `class`, `id` and `href` of math-core's own stylesheet and
    /// links. Left out.
    const DROPPED: &[&str] = &["style", "class", "id", "href"];
}

impl AsRef<str> for Mathml {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Why math-core's output was not written: markup the allowlist does not hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Refused {
    /// An element that is not `MathML`.
    Element(String),
    /// An attribute that is not a presentation attribute.
    Attribute {
        /// The element it is on.
        element: String,
        /// The attribute.
        attribute: String,
    },
    /// Markup that does not read as the allowlist's: a stray `<`, a bare `&`, an unclosed or
    /// misnested element, at this byte.
    Markup(usize),
}

impl fmt::Display for Refused {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Element(element) => write!(out, "the converter wrote `<{element}>`, which is not MathML"),
            Self::Attribute { element, attribute } => {
                write!(out, "the converter wrote `{attribute}` on `<{element}>`, which a page does not hold")
            }
            Self::Markup(at) => write!(out, "the converter wrote markup that does not read as MathML, at byte {at}"),
        }
    }
}

impl std::error::Error for Refused {}

impl TryFrom<&str> for Mathml {
    type Error = Refused;

    fn try_from(markup: &str) -> Result<Self, Refused> {
        let mut read = Read { markup, at: 0, out: String::with_capacity(markup.len()), open: Vec::new() };
        read.all()?;
        Ok(Self(read.out))
    }
}

/// The walk over math-core's output: where it is, what it kept, and the elements open.
#[derive(Debug)]
struct Read<'a> {
    markup: &'a str,
    at: usize,
    out: String,
    /// The elements open, the left-out ones among them.
    open: Vec<&'a str>,
}

impl<'a> Read<'a> {
    fn all(&mut self) -> Result<(), Refused> {
        while let Some(rest) = self.markup.get(self.at..).filter(|rest| !rest.is_empty()) {
            if let Some(tag) = rest.strip_prefix("</") {
                self.close(tag)?;
            } else if let Some(tag) = rest.strip_prefix('<') {
                self.open(tag)?;
            } else {
                self.text(rest)?;
            }
        }
        if self.open.is_empty() { Ok(()) } else { Err(Refused::Markup(self.at)) }
    }

    /// The name at the start of `rest`: lower-case letters, digits and `-`.
    fn name(rest: &str) -> &str {
        let end =
            rest.find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')).unwrap_or(rest.len());
        rest.get(..end).unwrap_or_default()
    }

    /// Whether `name` is an element kept, left out, or refused.
    fn element(name: &str) -> Result<bool, Refused> {
        if Mathml::ELEMENTS.contains(&name) {
            Ok(true)
        } else if Mathml::UNWRAPPED.contains(&name) {
            Ok(false)
        } else {
            Err(Refused::Element(name.to_owned()))
        }
    }

    /// `</name>`, closing the element last opened.
    fn close(&mut self, tag: &'a str) -> Result<(), Refused> {
        let name = Self::name(tag);
        if !tag.get(name.len()..).is_some_and(|rest| rest.starts_with('>')) || self.open.pop() != Some(name) {
            return Err(Refused::Markup(self.at));
        }
        if Self::element(name)? {
            self.out.push_str("</");
            self.out.push_str(name);
            self.out.push('>');
        }
        self.at += name.len() + 3;
        Ok(())
    }

    /// `<name attribute="value" ...>` or `<name .../>`, its attributes through the allowlist.
    fn open(&mut self, tag: &'a str) -> Result<(), Refused> {
        let name = Self::name(tag);
        if name.is_empty() {
            return Err(Refused::Markup(self.at));
        }
        let kept = Self::element(name)?;
        if kept {
            self.out.push('<');
            self.out.push_str(name);
        }
        let mut rest = tag.get(name.len()..).unwrap_or_default();
        loop {
            let trimmed = rest.trim_start_matches(' ');
            if let Some(after) = trimmed.strip_prefix("/>") {
                if kept {
                    self.out.push_str("/>");
                }
                rest = after;
                break;
            }
            if let Some(after) = trimmed.strip_prefix('>') {
                if kept {
                    self.out.push('>');
                }
                self.open.push(name);
                rest = after;
                break;
            }
            if trimmed.len() == rest.len() {
                return Err(Refused::Markup(self.at));
            }
            rest = self.attribute(name, kept, trimmed)?;
        }
        self.at = self.markup.len() - rest.len();
        Ok(())
    }

    /// One `attribute="value"` of `element` at the start of `rest`; gives back what follows.
    fn attribute(&mut self, element: &str, kept: bool, rest: &'a str) -> Result<&'a str, Refused> {
        let attribute = Self::name(rest);
        let at = self.markup.len() - rest.len();
        let value =
            rest.get(attribute.len()..).and_then(|after| after.strip_prefix("=\"")).ok_or(Refused::Markup(at))?;
        let end = value.find('"').ok_or(Refused::Markup(at))?;
        let written = value.get(..end).unwrap_or_default();
        if attribute.is_empty() || written.contains('<') || !Self::is_escaped(written) {
            return Err(Refused::Markup(at));
        }
        if Mathml::ATTRIBUTES.contains(&attribute) {
            if kept {
                self.out.push(' ');
                self.out.push_str(attribute);
                self.out.push_str("=\"");
                self.out.push_str(written);
                self.out.push('"');
            }
        } else if !Mathml::DROPPED.contains(&attribute) {
            return Err(Refused::Attribute { element: element.to_owned(), attribute: attribute.to_owned() });
        }
        Ok(value.get(end + 1..).unwrap_or_default())
    }

    /// Text up to the next `<`, kept when every `&` in it starts a reference.
    fn text(&mut self, rest: &str) -> Result<(), Refused> {
        let end = rest.find('<').unwrap_or(rest.len());
        let text = rest.get(..end).unwrap_or_default();
        if !Self::is_escaped(text) || text.contains('>') {
            return Err(Refused::Markup(self.at));
        }
        self.out.push_str(text);
        self.at += end;
        Ok(())
    }

    /// Whether every `&` in `text` starts a reference: `&name;` or `&#digits;`.
    fn is_escaped(text: &str) -> bool {
        text.split('&').skip(1).all(|after| {
            after.split_once(';').is_some_and(|(reference, _)| {
                let name = reference.strip_prefix('#').unwrap_or(reference);
                !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric())
            })
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn read(markup: &str) -> Result<String, Refused> {
        Mathml::try_from(markup).map(|mathml| mathml.0)
    }

    #[test]
    fn keeps_mathml_and_leaves_out_decorations() {
        let written = r#"<math display="block"><mrow class="x"><mi mathvariant="normal" style="color: red">a</mi><mo>&lt;</mo><mspace width="1em"/></mrow><mtext><a href="javascript:alert(1)">t</a></mtext></math>"#;
        assert_eq!(
            read(written).unwrap(),
            r#"<math display="block"><mrow><mi mathvariant="normal">a</mi><mo>&lt;</mo><mspace width="1em"/></mrow><mtext>t</mtext></math>"#
        );
    }

    #[test]
    fn refuses_what_is_not_mathml() {
        assert_eq!(read("<math><img src=x onerror=alert(1)></math>"), Err(Refused::Element("img".into())));
        assert_eq!(read("<math><script>x</script></math>"), Err(Refused::Element("script".into())));
        let attribute = Refused::Attribute { element: "mi".into(), attribute: "onclick".into() };
        assert_eq!(read(r#"<math><mi onclick="x">a</mi></math>"#), Err(attribute));
        assert!(matches!(read("<math><mi>a & b</mi></math>"), Err(Refused::Markup(_))));
        assert!(matches!(read("<math><mi>a</math></mi>"), Err(Refused::Markup(_))));
        assert!(matches!(read("<math><mi>a</mi>"), Err(Refused::Markup(_))));
        assert!(matches!(read("<math><mi width=x>a</mi></math>"), Err(Refused::Markup(_))));
    }
}
