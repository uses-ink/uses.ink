//! The text colours of every look are readable: WCAG 2.2 AA, 4.5 to 1, on the paper and on the
//! tint, in light and in dark. Lines and borders carry no meaning of their own, so they have no
//! ratio to meet.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

use uses_ink_token::{Colour, Look, Rgb, Role, Scheme};

/// The relative luminance of WCAG 2.2: each channel linearised, then weighted.
fn luminance(Rgb(r, g, b): Rgb) -> f64 {
    let channel = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.03928 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

/// The contrast ratio of two colours, 1 to 21.
fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[test]
fn every_looks_text_meets_aa_on_its_grounds() {
    for look in Look::ALL {
        for scheme in Scheme::ALL {
            for text in [Role::Ink, Role::Muted, Role::Accent] {
                for ground in [Role::Paper, Role::Tint] {
                    let ratio = contrast(look.colour(scheme, text), look.colour(scheme, ground));
                    assert!(
                        ratio >= 4.5,
                        "{} {scheme:?}: {} on {} is {ratio:.2}:1, under the 4.5:1 of WCAG AA",
                        look.name(),
                        text.name(),
                        ground.name(),
                    );
                }
            }
        }
    }
}

#[test]
fn the_ratio_of_black_on_white_is_21() {
    assert!((contrast(Rgb(0, 0, 0), Rgb(255, 255, 255)) - 21.0).abs() < 1e-9);
}

/// Whether a look has colours of its own for a dark setting: its stylesheet then says so.
fn has_dark(look: Look) -> bool {
    look.css().contains("@media (prefers-color-scheme: dark)")
}

#[test]
fn a_look_declares_each_role_in_light_and_again_in_dark_when_it_has_one() {
    for look in Look::ALL {
        let css = look.css();
        let times = if has_dark(*look) { 2 } else { 1 };
        for role in Role::ALL {
            assert_eq!(
                css.matches(&format!("--{}:", role.name())).count(),
                times,
                "{} holds {} once per scheme it has",
                look.name(),
                role.name()
            );
        }
        let kept = look.colour(Scheme::Light, Role::Paper) == look.colour(Scheme::Dark, Role::Paper);
        assert_eq!(
            kept,
            !has_dark(*look),
            "{}: a look with no dark scheme keeps its paper, one with a scheme changes it",
            look.name()
        );
    }
}

#[test]
fn a_look_declares_the_hand_and_the_paint_box_beside_its_roles() {
    for look in Look::ALL {
        let css = look.css();
        for name in
            ["--face-body:", "--face-hand:", "--face-mono:", "--pen:", "--bucket:", "--sky:", "--mint:", "--pink:"]
        {
            assert!(css.contains(name), "{} declares {name}", look.name());
        }
    }
}

/// The pencil is the text on every fill, and red is only ever set on paper: red on the bucket
/// yellow is 4.29:1, under AA, so a look must not need it there.
#[test]
fn the_pencil_reads_on_each_fill_of_the_paint_box() {
    let ink = Colour::Ink.rgb();
    for fill in [Colour::Bucket, Colour::Sky, Colour::Mint, Colour::Pink] {
        let ratio = contrast(ink, fill.rgb());
        assert!(ratio >= 4.5, "{} on {} is {ratio:.2}:1", Colour::Ink.name(), fill.name());
    }
}

#[test]
fn a_name_that_is_no_look_says_which_there_are() {
    let refused = Look::named("loud").unwrap_err();
    assert_eq!(refused.to_string(), "no look named `loud`; the looks are `uses-ink`");
    assert_eq!(Look::named("uses-ink").unwrap(), Look::UsesInk);
}
