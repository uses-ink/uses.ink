//! Raster images: the host prepares each one before any page is rendered; its copies are served
//! at routes holding the hash of their bytes, never cached stale; the original is not served; a
//! kept image is served as it is and said; a refused one is said and not served.

use engine::image::{Prepared, Source};
use engine::{Engine, Images, NoPins, Rendered, Theme};
use model::{Diagnostic, Hash, MediaType, Page, Picture, Route, Site};

use crate::support::{Pinned, Plain, Tree, settings};

/// A page showing `image.png`, whose bytes are `bytes`.
fn showing(bytes: &str) -> Tree {
    Tree::new().file("index.md", "![a photo](image.png)\n").file("image.png", bytes)
}

fn route(text: &str) -> Route {
    text.parse().unwrap()
}

/// A theme that writes each page's pictures as the engine gave them.
#[derive(Debug, Default)]
struct Pictures;

impl Theme for Pictures {
    fn name(&self) -> &'static str {
        "pictures"
    }

    fn assets(&self) -> Vec<engine::File> {
        Vec::new()
    }

    fn render<R: engine::Sink + ?Sized>(
        &self,
        _page: &Page,
        site: &Site,
        _record: &mut R,
    ) -> Result<engine::Html, Diagnostic> {
        let pictures: Vec<&Picture> = site.assets().iter().filter_map(|asset| asset.picture.as_ref()).collect();
        Ok(format!("{pictures:?}").into())
    }
}

fn picture(rendered: &Rendered) -> String {
    let entry = rendered.manifest.entry(&Route::root()).unwrap();
    String::from_utf8(rendered.blobs[&entry.blob()].clone()).unwrap()
}

#[test]
fn an_image_is_served_as_copies_named_by_their_hash() {
    let rendered = showing("photo").render(&Plain::default());
    assert!(rendered.diagnostics.is_empty(), "{:?}", rendered.diagnostics);
    assert!(rendered.manifest.entry(&route("/image.png")).is_none(), "the original keeps its metadata");
    let copies: Vec<_> =
        rendered.manifest.entries().iter().filter(|e| e.route().as_str().starts_with("/image.")).collect();
    let media: Vec<MediaType> = copies.iter().map(|e| e.media()).collect();
    assert_eq!(media.len(), 5);
    for copy in copies {
        let bytes = &rendered.blobs[&copy.blob()];
        let hex = Hash::of(bytes).to_string();
        assert!(copy.route().as_str().contains(&hex[..16]), "{}", copy.route());
        let headers: Vec<(&str, &str)> = copy.headers().iter().map(|h| (h.name.name(), h.value.as_str())).collect();
        let expected = [
            ("cache-control", "public, max-age=31536000, immutable"),
            ("content-security-policy", "default-src 'none'; frame-ancestors 'none'; sandbox"),
            ("cross-origin-resource-policy", "same-origin"),
            ("x-content-type-options", "nosniff"),
        ];
        assert_eq!(headers, expected);
    }
    let avif = String::from_utf8(rendered.blobs.values().find(|b| b.ends_with(b"image/avif 40")).unwrap().clone());
    assert_eq!(avif.unwrap(), "photo image/avif 40");
}

#[test]
fn a_theme_sees_the_size_copies_and_placeholder() {
    let rendered = showing("photo").render(&Pictures);
    let shown = picture(&rendered);
    assert!(shown.starts_with("[Picture { dimensions: Dimensions { width: 40, height: 30 }"), "{shown}");
    let order: Vec<&str> = shown.match_indices("media: ").map(|(at, _)| &shown[at + 7..at + 11]).collect();
    assert_eq!(order, ["Jpeg", "Jpeg", "Avif", "Avif"], "by format, then width");
    assert!(shown.contains("placeholder: Some(Route(\"/image.placeholder."), "{shown}");
}

#[test]
fn an_image_with_transparency_has_no_placeholder() {
    let shown = picture(&showing("clear").render(&Pictures));
    assert!(shown.ends_with("placeholder: None }]"), "{shown}");
}

#[test]
fn an_animated_image_is_kept_as_it_is_and_said() {
    let rendered = showing("animated").render(&Pictures);
    assert!(rendered.manifest.entry(&route("/image.png")).is_some());
    let said: Vec<&str> =
        rendered.notices.iter().map(model::Notice::code).filter(|c| c.starts_with("image.")).collect();
    assert_eq!(said, ["image.animation.kept"]);
    assert!(picture(&rendered).contains("variants: [], placeholder: None"));
}

#[test]
fn a_refused_image_is_said_on_its_file_and_not_served() {
    let rendered = showing("broken").render(&Plain::default());
    let said: Vec<(&str, String)> =
        rendered.diagnostics.iter().map(|d| (d.code(), d.subject.as_ref().unwrap().to_string())).collect();
    assert_eq!(said, [("image.decode.failed", "image.png".to_owned())]);
    assert!(!rendered.manifest.entries().iter().any(|e| e.route().as_str().starts_with("/image.")));
}

/// A host that loses every image it is given.
#[derive(Debug)]
struct Lossy;

impl Images for Lossy {
    type Refusal = Diagnostic;

    fn prepare(&self, _sources: &[Source<'_>]) -> Vec<Result<Prepared, Diagnostic>> {
        Vec::new()
    }
}

#[test]
fn a_host_that_loses_images_is_a_bug_said() {
    let tree = showing("photo");
    let (_, store) = tree.snapshot();
    let site = tree.understand(&Pinned::none()).site.unwrap();
    let rendered = Engine::new(&store, NoPins, (), Plain::default()).render(&site, &settings(), &Lossy);
    let codes: Vec<&str> = rendered.diagnostics.iter().map(Diagnostic::code).collect();
    assert_eq!(codes, ["engine.image.miscounted"]);
}

#[test]
fn the_same_images_give_the_same_deploy() {
    let first = showing("photo").render(&Plain::default());
    let second = showing("photo").render(&Plain::default());
    assert_eq!(first.manifest.id(), second.manifest.id());
    assert_ne!(first.manifest.id(), showing("photograph").render(&Plain::default()).manifest.id());
}
