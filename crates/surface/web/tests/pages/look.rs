//! The dashboard's look: every file its pages borrow is served and drawn, every page names them,
//! the navigation is one landmark with one current tab, every mark is hidden from a reader, and
//! the text colours the stylesheet sets meet the contrast the decision promises.

use super::structure::Structure;
use super::{Fixtures, Pageable};
use token::{Colour, Rgb};
use uses_ink_web::view::{Account, SignIn};
use uses_ink_web::{Asset, Medium, Reply};

/// A colour with the contrast it makes against another, by WCAG 2's formula.
struct Shade(Rgb);

impl Shade {
    fn luminance(&self) -> f64 {
        let Rgb(r, g, b) = self.0;
        let linear = |channel: u8| {
            let c = f64::from(channel) / 255.0;
            if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
    }

    fn against(&self, ground: &Self) -> f64 {
        let (a, b) = (self.luminance(), ground.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
}

impl From<Colour> for Shade {
    fn from(colour: Colour) -> Self {
        Self(colour.rgb())
    }
}

#[test]
fn every_page_names_every_file_it_borrows_and_each_is_served() {
    for (name, page) in Fixtures::pages() {
        for asset in Asset::ALL {
            assert!(page.contains(asset.path()), "{name}: {}", asset.path());
        }
    }
    for asset in Asset::ALL {
        let bytes = asset.bytes();
        assert!(!bytes.is_empty(), "{}", asset.path());
        match asset.medium() {
            Medium::Font => assert!(bytes.starts_with(b"wOF2"), "{} is woff2", asset.path()),
            Medium::Image => assert!(String::from_utf8_lossy(&bytes).contains("<svg"), "{} is an svg", asset.path()),
            _ => assert!(String::from_utf8_lossy(&bytes).contains(":root"), "{} holds the tokens", asset.path()),
        }
        assert_eq!(Asset::named(asset.file()), Some(asset), "{} is found by its name", asset.path());
    }
}

#[test]
fn the_strokes_are_drawn_the_same_every_time_and_the_three_hands_differ() {
    let drawn: Vec<Vec<u8>> =
        [Asset::Outline(uses_ink_web::Wobble::First), Asset::Outline(uses_ink_web::Wobble::Second)]
            .iter()
            .map(|asset| asset.bytes().into_owned())
            .collect();
    assert_eq!(drawn[0], Asset::Outline(uses_ink_web::Wobble::First).bytes().into_owned());
    assert_ne!(drawn[0], drawn[1]);
}

#[test]
fn the_policy_lets_the_page_load_its_faces_and_its_strokes_from_itself_and_nothing_else() {
    let csp =
        Reply::<super::routing::Replay>::HEADERS.iter().find(|(name, _)| *name == "content-security-policy").unwrap().1;
    assert!(
        csp.contains("font-src 'self'")
            && csp.contains("img-src 'self'")
            && csp.contains("style-src 'self' 'unsafe-inline'"),
        "{csp}"
    );
    assert!(
        csp.contains("default-src 'none'") && !csp.contains("data:") && !csp.contains("script-src 'unsafe-inline'"),
        "{csp}"
    );
}

#[test]
fn the_navigation_is_one_named_landmark_with_at_most_one_current_tab_and_every_mark_is_hidden() {
    for (name, page) in Fixtures::pages() {
        let structure = Structure(&page);
        assert_eq!(structure.tags("nav").len(), 1, "{name}: one nav");
        assert!(structure.tags("nav")[0].contains("aria-label=\"Screens\""), "{name}: the nav has a name");
        assert!(
            structure.tags("a").iter().filter(|tag| tag.contains("aria-current")).count() <= 1,
            "{name}: at most one current tab"
        );
        assert_eq!(structure.tags("main").len(), 1, "{name}: one main");
        assert_eq!(structure.tags("header").len(), 1, "{name}: one banner");
        for img in structure.tags("img") {
            assert!(img.contains("alt=\"\""), "{name}: a mark has no description: {img}");
        }
        for svg in structure.tags("svg") {
            assert!(svg.contains("aria-hidden=\"true\""), "{name}: a drawn mark is hidden: {svg}");
        }
    }
    let account = Account::new(Fixtures::at("2026-11-01T12:00:00Z"), vec![]).page();
    assert_eq!(
        Structure(&account).tags("a").iter().filter(|tag| tag.contains("aria-current")).count(),
        1,
        "the account page marks its own tab"
    );
    let signin = SignIn::new().page();
    assert_eq!(Structure(&signin).tags("a").iter().filter(|tag| tag.contains("aria-current")).count(), 0);
}

#[test]
fn the_text_colours_the_sheet_sets_meet_aa_on_the_grounds_they_sit_on() {
    let grounds = [Colour::Paper, Colour::Bucket, Colour::Sky, Colour::Mint, Colour::Pink, Colour::Cloud];
    for ground in grounds {
        let ratio = Shade::from(Colour::Ink).against(&Shade::from(ground));
        assert!(ratio >= 4.5, "ink on {}: {ratio}", ground.name());
    }
    for ground in [Colour::Paper, Colour::Cloud, Colour::Bucket, Colour::Mint, Colour::Sky] {
        let ratio = Shade::from(Colour::Pencil).against(&Shade::from(ground));
        assert!(ratio >= 4.5, "pencil on {}: {ratio}", ground.name());
    }
    // the red pen is text only on paper: on a fill it is not readable, so the sheet never sets it there
    let pen = Shade::from(Colour::Pen).against(&Shade::from(Colour::Paper));
    assert!(pen >= 4.5, "pen on paper: {pen}");
    assert!(
        Shade::from(Colour::Pen).against(&Shade::from(Colour::Pink)) < 4.5,
        "the rule above is why red stays off pink"
    );
}

#[test]
fn the_served_tokens_set_every_colour_and_face_the_sheet_reads_to_its_own_value() {
    let css = String::from_utf8(Asset::Tokens.bytes().into_owned()).unwrap();
    for colour in Colour::ALL {
        assert!(css.contains(&format!("--{}: {};", colour.name(), colour.rgb())), "--{}", colour.name());
    }
    for face in token::Face::ALL {
        assert!(css.contains(&format!("--{}:", face.name())), "--{}", face.name());
    }
}
