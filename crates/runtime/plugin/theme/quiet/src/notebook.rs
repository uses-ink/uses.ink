//! The files of the hand-drawn look (`look = "uses-ink"`, decision 0180): the stylesheet of the
//! doodle design pass, its faces and the marks the pen drew for it. Every mark is a file that
//! `cargo xtask brand` wrote from `brand/figure.toml`; the stylesheet lays them out with CSS, so a
//! page carries no script and draws itself even where nothing else loads.
//!
//! The classes the markup of the look uses are declared here and given the one rule each needs to
//! be a class; the stylesheet's own rules are `assets/notebook.css`, written after them.

use engine::File;
use model::{MediaType, Route};

unspliced::css! {
    pub(crate) mod hand;

    .panel { display: block; }
    .hero { display: grid; }
    .end { display: flex; }
    .say { margin: 0; }
    .ask { display: grid; }
    .field { display: block; }
    .go { margin: 0; }
    .sr { position: absolute; left: -9999px; }
    .show { margin: 0; }
    .logo { display: inline-block; }
    .made { display: inline-flex; }
}

/// One file of the look: where it is served, what it is and its bytes.
#[derive(Clone, Copy, Debug)]
struct Part {
    route: &'static str,
    media: MediaType,
    bytes: &'static [u8],
}

/// The hand-drawn look's files, as the theme serves them (the notebook of decision 0016).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Notebook;

impl Notebook {
    /// The stylesheet's route, linked after the look's custom properties.
    pub(crate) const SHEET: &'static str = "/notebook.css";

    /// The stylesheet, the marks and the faces, in the order they are listed.
    const PARTS: &'static [Part] = &[
        Part {
            route: "/fonts/gaegu-regular.woff2",
            media: MediaType::Woff2,
            bytes: include_bytes!("../assets/fonts/Gaegu-Regular.woff2"),
        },
        Part {
            route: "/fonts/gaegu-bold.woff2",
            media: MediaType::Woff2,
            bytes: include_bytes!("../assets/fonts/Gaegu-Bold.woff2"),
        },
        Part {
            route: "/fonts/literata.woff2",
            media: MediaType::Woff2,
            bytes: include_bytes!("../assets/fonts/Literata.woff2"),
        },
        Part {
            route: "/fonts/literata-italic.woff2",
            media: MediaType::Woff2,
            bytes: include_bytes!("../assets/fonts/Literata-Italic.woff2"),
        },
        Part {
            route: "/fonts/monaspace-radon.woff2",
            media: MediaType::Woff2,
            bytes: include_bytes!("../assets/fonts/MonaspaceRadon-Regular.woff2"),
        },
        Part { route: "/marks/rule.svg", media: MediaType::Svg, bytes: include_bytes!("../assets/marks/rule.svg") },
        Part {
            route: "/marks/underline-ink.svg",
            media: MediaType::Svg,
            bytes: include_bytes!("../assets/marks/underline-ink.svg"),
        },
        Part {
            route: "/marks/underline-pen.svg",
            media: MediaType::Svg,
            bytes: include_bytes!("../assets/marks/underline-pen.svg"),
        },
        Part {
            route: "/marks/highlight.svg",
            media: MediaType::Svg,
            bytes: include_bytes!("../assets/marks/highlight.svg"),
        },
        Part { route: "/marks/bullet.svg", media: MediaType::Svg, bytes: include_bytes!("../assets/marks/bullet.svg") },
        Part { route: "/marks/circle.svg", media: MediaType::Svg, bytes: include_bytes!("../assets/marks/circle.svg") },
        Part { route: "/marks/frame.svg", media: MediaType::Svg, bytes: include_bytes!("../assets/marks/frame.svg") },
        Part { route: "/marks/mint.svg", media: MediaType::Svg, bytes: include_bytes!("../assets/marks/mint.svg") },
        Part { route: "/marks/tape.svg", media: MediaType::Svg, bytes: include_bytes!("../assets/marks/tape.svg") },
        Part {
            route: "/marks/arrow-down.svg",
            media: MediaType::Svg,
            bytes: include_bytes!("../assets/marks/arrow-down.svg"),
        },
        Part {
            route: "/marks/note-arrow.svg",
            media: MediaType::Svg,
            bytes: include_bytes!("../assets/marks/note-arrow.svg"),
        },
        Part { route: "/marks/tick.svg", media: MediaType::Svg, bytes: include_bytes!("../assets/marks/tick.svg") },
        Part {
            route: "/marks/mark.svg",
            media: MediaType::Svg,
            bytes: include_bytes!("../../../../../../brand/mark.svg"),
        },
        Part {
            route: "/marks/logo-open.svg",
            media: MediaType::Svg,
            bytes: include_bytes!("../../../../../../brand/logo-open.svg"),
        },
    ];

    /// The stylesheet: the classes of [`hand`], then the rules of `assets/notebook.css`.
    fn sheet() -> Vec<u8> {
        let rules = include_str!("../assets/notebook.css");
        format!("{}\n{rules}", hand::SHEET.text()).into_bytes()
    }

    /// Every file of the look. A route that does not parse is a bug in the list above and is left
    /// out; a test holds the list to its routes.
    pub(crate) fn files() -> Vec<File> {
        let sheet =
            Self::SHEET.parse::<Route>().ok().map(|route| File { route, media: MediaType::Css, bytes: Self::sheet() });
        let parts = Self::PARTS.iter().filter_map(|part| {
            part.route.parse::<Route>().ok().map(|route| File { route, media: part.media, bytes: part.bytes.to_vec() })
        });
        sheet.into_iter().chain(parts).collect()
    }
}

#[cfg(test)]
mod test {
    use super::Notebook;

    #[test]
    fn every_part_of_the_look_has_a_route_and_bytes() {
        let files = Notebook::files();
        assert_eq!(files.len(), Notebook::PARTS.len() + 1, "a route of the look does not parse");
        assert!(files.iter().all(|file| !file.bytes.is_empty()));
    }
}
