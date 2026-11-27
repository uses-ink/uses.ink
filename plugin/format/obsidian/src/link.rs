//! Wikilinks: what is between `[[` and `]]` read as Obsidian reads it, and what each becomes.

use std::fmt;

use format_markdown::dialect::{Linked, Plain, Wikilink};
use model::{Announcement, ImageSource, Inline, LinkTarget};

/// A wikilink's target: a note or a file, and the heading or block of it named after a `#`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Target<'a> {
    /// The note or file, as written; empty for a place on the same page (`[[#Heading]]`).
    path: &'a str,
    /// The last `#` part: a heading's text, or a block id with its `^`.
    anchor: Option<&'a str>,
}

/// What kind of file a target names, by its extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// A note: `.md`, or no extension Obsidian gives a file of another kind.
    Note,
    /// An image a page can show.
    Image,
    /// A file Obsidian embeds with a player or a viewer: a PDF, audio, video, a canvas.
    File,
}

impl Kind {
    /// The extensions of images, in lower case.
    const IMAGES: &'static [&'static str] = &["avif", "bmp", "gif", "jpeg", "jpg", "png", "svg", "webp"];

    /// The extensions of the other files Obsidian embeds, in lower case.
    const FILES: &'static [&'static str] = &[
        "3gp", "base", "canvas", "flac", "m4a", "mkv", "mov", "mp3", "mp4", "oga", "ogg", "ogv", "opus", "pdf", "wav",
        "webm",
    ];
}

impl<'a> From<&'a str> for Target<'a> {
    /// `path#a#b` names `b` of `path`; spaces around either part are not part of it.
    fn from(written: &'a str) -> Self {
        match written.split_once('#') {
            Some((path, anchors)) => {
                let anchor = anchors.rsplit('#').next().map(str::trim).filter(|anchor| !anchor.is_empty());
                Self { path: path.trim(), anchor }
            }
            None => Self { path: written.trim(), anchor: None },
        }
    }
}

impl<'a> Target<'a> {
    /// What the wikilink `link` to this target becomes.
    pub(crate) fn linked(self, link: Wikilink<'_>) -> Linked {
        let aliased = Plain::of(&link.label) != link.target;
        let label = if aliased { link.label } else { vec![Inline::Text(Shown(self).to_string())] };
        let written = self.to_string();
        match (link.embedded, self.kind()) {
            (false, _) => Inline::Link { target: LinkTarget::Unresolved { written }, children: label }.into(),
            (true, Kind::Image) => self.image(aliased.then(|| Plain::of(&label)).as_deref()),
            (true, Kind::Note) => {
                let inline =
                    Inline::Link { target: LinkTarget::Unresolved { written: written.clone() }, children: label };
                Linked { inline, embed: Some(written), said: None }
            }
            (true, Kind::File) => {
                let inline =
                    Inline::Link { target: LinkTarget::Unresolved { written: written.clone() }, children: label };
                Linked { inline, embed: None, said: Some(Announcement::EmbedFileLinked { written }) }
            }
        }
    }

    /// An embedded image: its alt text is `after`, the text after the `|`, or the file name; a
    /// size after the `|` is said and not kept.
    fn image(self, after: Option<&str>) -> Linked {
        let size = after.filter(|after| Size(after).is_size());
        let alt = after.filter(|after| size.is_none() && !after.trim().is_empty()).map_or(self.name(), str::trim);
        let source = ImageSource::Unresolved { written: self.path.to_owned() };
        let said = size.map(|size| Announcement::EmbedSizeIgnored { written: size.to_owned() });
        Linked { inline: Inline::Image { source, alt: alt.to_owned() }, embed: None, said }
    }

    /// The file name: the path after its last `/`.
    fn name(self) -> &'a str {
        self.path.rsplit('/').next().unwrap_or(self.path)
    }

    fn kind(self) -> Kind {
        let extension = self.name().rsplit_once('.').map(|(_, extension)| extension.to_ascii_lowercase());
        match extension.as_deref() {
            Some(extension) if Kind::IMAGES.contains(&extension) => Kind::Image,
            Some(extension) if Kind::FILES.contains(&extension) => Kind::File,
            _ => Kind::Note,
        }
    }
}

impl fmt::Display for Target<'_> {
    /// The target as the engine resolves it: `path`, `path#anchor` or `#anchor`.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.path)?;
        match self.anchor {
            Some(anchor) => write!(out, "#{anchor}"),
            None => Ok(()),
        }
    }
}

/// A target as Obsidian shows a wikilink without a `|`: `Note`, `Note > Heading`, `Heading`.
#[derive(Clone, Copy, Debug)]
struct Shown<'a>(Target<'a>);

impl fmt::Display for Shown<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Target { path, anchor } = self.0;
        let path = path.strip_suffix(".md").unwrap_or(path);
        match anchor {
            Some(anchor) if path.is_empty() => out.write_str(anchor),
            Some(anchor) => write!(out, "{path} > {anchor}"),
            None => out.write_str(path),
        }
    }
}

/// Text after a `|` that may be an image's size: `300` or `300x200`.
#[derive(Clone, Copy, Debug)]
struct Size<'a>(&'a str);

impl Size<'_> {
    fn is_size(self) -> bool {
        let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
        let size = self.0.trim();
        match size.split_once('x') {
            Some((width, height)) => digits(width) && digits(height),
            None => digits(size),
        }
    }
}

#[cfg(test)]
mod test {
    use super::Target;

    #[test]
    fn the_last_anchor_is_the_one_named() {
        assert_eq!(Target::from("Note#A#B").to_string(), "Note#B");
        assert_eq!(Target::from(" Note # Part ").to_string(), "Note#Part");
        assert_eq!(Target::from("#^block").to_string(), "#^block");
        assert_eq!(Target::from("Note#").to_string(), "Note");
    }
}
