// Highlighting, the files a theme serves, and the pen. Part of `wit/mirror`.

use model::MediaType;
use model::highlight::{Capture, Run, Unhighlighted};

use super::refusal::{Malformed, Unmirrored};
use super::wit::host::{highlight as shade, pen};
use super::wit::model::file as served;
use super::written::Written;

impl TryFrom<Capture> for shade::Capture {
    type Error = Unmirrored;

    fn try_from(capture: Capture) -> Result<Self, Unmirrored> {
        Ok(match capture {
            Capture::Attribute => Self::Attribute,
            Capture::Comment => Self::Comment,
            Capture::Constant => Self::Constant,
            Capture::Constructor => Self::Constructor,
            Capture::Deleted => Self::Deleted,
            Capture::Embedded => Self::Embedded,
            Capture::Emphasis => Self::Emphasis,
            Capture::Error => Self::Error,
            Capture::Function => Self::Function,
            Capture::Heading => Self::Heading,
            Capture::Inserted => Self::Inserted,
            Capture::Keyword => Self::Keyword,
            Capture::Label => Self::Label,
            Capture::Link => Self::Link,
            Capture::Literal => Self::Literal,
            Capture::Macro => Self::Macro,
            Capture::Module => Self::Module,
            Capture::Number => Self::Number,
            Capture::Operator => Self::Operator,
            Capture::Property => Self::Property,
            Capture::Punctuation => Self::Punctuation,
            Capture::Strike => Self::Strike,
            Capture::String => Self::String,
            Capture::Strong => Self::Strong,
            Capture::Tag => Self::Tag,
            Capture::Type => Self::Type,
            Capture::Variable => Self::Variable,
            _ => return Err(Unmirrored::Capture),
        })
    }
}

impl From<shade::Capture> for Capture {
    fn from(capture: shade::Capture) -> Self {
        match capture {
            shade::Capture::Attribute => Self::Attribute,
            shade::Capture::Comment => Self::Comment,
            shade::Capture::Constant => Self::Constant,
            shade::Capture::Constructor => Self::Constructor,
            shade::Capture::Deleted => Self::Deleted,
            shade::Capture::Embedded => Self::Embedded,
            shade::Capture::Emphasis => Self::Emphasis,
            shade::Capture::Error => Self::Error,
            shade::Capture::Function => Self::Function,
            shade::Capture::Heading => Self::Heading,
            shade::Capture::Inserted => Self::Inserted,
            shade::Capture::Keyword => Self::Keyword,
            shade::Capture::Label => Self::Label,
            shade::Capture::Link => Self::Link,
            shade::Capture::Literal => Self::Literal,
            shade::Capture::Macro => Self::Macro,
            shade::Capture::Module => Self::Module,
            shade::Capture::Number => Self::Number,
            shade::Capture::Operator => Self::Operator,
            shade::Capture::Property => Self::Property,
            shade::Capture::Punctuation => Self::Punctuation,
            shade::Capture::Strike => Self::Strike,
            shade::Capture::String => Self::String,
            shade::Capture::Strong => Self::Strong,
            shade::Capture::Tag => Self::Tag,
            shade::Capture::Type => Self::Type,
            shade::Capture::Variable => Self::Variable,
        }
    }
}

impl TryFrom<Run<'_>> for shade::Run {
    type Error = Unmirrored;

    fn try_from(run: Run<'_>) -> Result<Self, Unmirrored> {
        let length = u32::try_from(run.text.len()).map_err(|_| Unmirrored::Large)?;
        Ok(Self { capture: run.capture.map(shade::Capture::try_from).transpose()?, length })
    }
}

/// Runs from the WIT over the code they cover.
pub(crate) struct Covered<'c>(pub(crate) &'c str, pub(crate) Vec<shade::Run>);

impl<'c> TryFrom<Covered<'c>> for Vec<Run<'c>> {
    type Error = Malformed;

    /// Each run's text sliced off the code in order; the runs must end at its end, on
    /// character boundaries.
    fn try_from(Covered(code, runs): Covered<'c>) -> Result<Self, Malformed> {
        let mut rest = code;
        let mut made = Vec::with_capacity(runs.len());
        for run in runs {
            let length = usize::try_from(run.length).map_err(|_| Malformed::Runs)?;
            let (Some(text), Some(after)) = (rest.get(..length), rest.get(length..)) else {
                return Err(Malformed::Runs);
            };
            made.push(Run { capture: run.capture.map(Capture::from), text });
            rest = after;
        }
        if rest.is_empty() { Ok(made) } else { Err(Malformed::Runs) }
    }
}

impl<E> From<&Unhighlighted<E>> for shade::Unhighlighted {
    fn from(unhighlighted: &Unhighlighted<E>) -> Self {
        match unhighlighted {
            Unhighlighted::Unknown => Self::Unknown,
            _ => Self::Failed,
        }
    }
}

// What a theme serves.

impl TryFrom<MediaType> for served::Media {
    type Error = Unmirrored;

    fn try_from(media: MediaType) -> Result<Self, Unmirrored> {
        Ok(match media {
            MediaType::Html => Self::Html,
            MediaType::Css => Self::Css,
            MediaType::Xml => Self::Xml,
            MediaType::Atom => Self::Atom,
            MediaType::Rss => Self::Rss,
            MediaType::Json => Self::Json,
            MediaType::Text => Self::Text,
            MediaType::Svg => Self::Svg,
            MediaType::Png => Self::Png,
            MediaType::Jpeg => Self::Jpeg,
            MediaType::Avif => Self::Avif,
            MediaType::Webp => Self::Webp,
            MediaType::Gif => Self::Gif,
            MediaType::Ico => Self::Ico,
            MediaType::Woff2 => Self::Woff2,
            MediaType::Pdf => Self::Pdf,
            _ => return Err(Unmirrored::Media),
        })
    }
}

impl From<served::Media> for MediaType {
    fn from(media: served::Media) -> Self {
        match media {
            served::Media::Html => Self::Html,
            served::Media::Css => Self::Css,
            served::Media::Xml => Self::Xml,
            served::Media::Atom => Self::Atom,
            served::Media::Rss => Self::Rss,
            served::Media::Json => Self::Json,
            served::Media::Text => Self::Text,
            served::Media::Svg => Self::Svg,
            served::Media::Png => Self::Png,
            served::Media::Jpeg => Self::Jpeg,
            served::Media::Avif => Self::Avif,
            served::Media::Webp => Self::Webp,
            served::Media::Gif => Self::Gif,
            served::Media::Ico => Self::Ico,
            served::Media::Woff2 => Self::Woff2,
            served::Media::Pdf => Self::Pdf,
        }
    }
}

impl TryFrom<model::File> for served::File {
    type Error = Unmirrored;

    fn try_from(file: model::File) -> Result<Self, Unmirrored> {
        Ok(Self { route: file.route.into(), media: file.media.try_into()?, bytes: file.bytes })
    }
}

impl TryFrom<served::File> for model::File {
    type Error = Malformed;

    fn try_from(file: served::File) -> Result<Self, Malformed> {
        let route = Written::read(file.route).map_err(|source| Malformed::File { source: Box::new(source) })?;
        Ok(Self { route, media: file.media.into(), bytes: file.bytes })
    }
}

// The pen.

impl From<pen::Point> for freehand::Point {
    fn from(point: pen::Point) -> Self {
        Self::new(point.x, point.y)
    }
}

impl From<freehand::Point> for pen::Point {
    fn from(point: freehand::Point) -> Self {
        Self { x: point.x, y: point.y }
    }
}

impl From<pen::Rectangle> for freehand::Rectangle {
    fn from(rectangle: pen::Rectangle) -> Self {
        Self { top_left: rectangle.top_left.into(), bottom_right: rectangle.bottom_right.into() }
    }
}

impl From<pen::Hand> for freehand::Hand {
    fn from(hand: pen::Hand) -> Self {
        Self { wobble: hand.wobble, tremor: hand.tremor }
    }
}

impl From<pen::Ellipse> for freehand::Ellipse {
    fn from(ellipse: pen::Ellipse) -> Self {
        Self {
            centre: ellipse.centre.into(),
            radius: ellipse.radius.into(),
            from: ellipse.start,
            turns: ellipse.turns,
            count: ellipse.count,
        }
    }
}
