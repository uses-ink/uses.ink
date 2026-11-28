//! Goldens of the output blocks, through the fixture harness (`cli/fixture/term/`): each block as
//! text without colour, with colour (escape written `\e` so a diff reads), with ascii glyphs and at
//! 40 columns, and each block that has a JSON form as JSON. The text of a block also holds every
//! field of its JSON, checked on the same value.

use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Parser as _;
use model::{Announcement, ContentPath, Diagnostic, Notice, Problem, Span, Subject};

use crate::command::publish::view::{Exported, Linked, Standing};
use crate::command::source::Source;
use crate::deploy::Failure as Refused;
use crate::deploy::Logged;
use crate::grammar::args::{Cli, Origin};
use crate::output::Tally;
use crate::output::error::Error;
use crate::output::failure::Failure;
use crate::output::report::{Event, Noted, Shown, Texts};
use crate::output::stage::{StageLine, Verb};
use crate::output::term::{
    Answer, Cut, Doc, Elapsed, Glyph, Glyphs, Line, Look, Phase, Rail, Record, Render, Role, Table, Theme, Tick, Tree,
    Trees, Width, Writer,
};
use control::JobState;
use edge_api::view::Status as Overview;

/// One way the output can look.
struct Shot {
    name: &'static str,
    look: Look,
}

impl Shot {
    /// The four looks of a golden.
    fn all() -> [Self; 4] {
        let plain = Look::PLAIN;
        [
            Self { name: "plain", look: plain },
            Self { name: "colour", look: Look { colour: true, ..plain } },
            Self { name: "ascii", look: Look { glyphs: Glyphs::Ascii, ..plain } },
            Self { name: "narrow", look: Look { width: Width::Columns(40), ..plain } },
        ]
    }

    /// `block` as this look writes it.
    fn text(&self, block: &impl Render) -> String {
        let mut out = Vec::new();
        Writer::new(&mut out, self.look).emit(block).unwrap();
        String::from_utf8(out).unwrap().replace('\x1b', "\\e")
    }
}

/// A path of a site.
fn path(text: &str) -> ContentPath {
    ContentPath::parse(text).unwrap()
}

/// The decisions of a source and what the explain tree of the readme fixture holds.
fn decisions() -> Trees {
    let row = |code: &str, value: &str, evidence: &str, role: Role| {
        vec![Line::new().with(Role::Code, code), Line::plain(value), Line::new().with(role, evidence)]
    };
    let source = Tree::new(Line::new().with(Role::Heading, "the source"))
        .row(vec![Line::new().with(Role::Quiet, "version"), Line::plain("the files on disk")])
        .row(vec![Line::new().with(Role::Quiet, "lacks"), Line::plain("history, so page.date is not taken from it")])
        .flex(1, Cut::End);
    let page = Tree::new(Line::new().with(Role::Heading, "posts/a.md"))
        .row(row("page.publish", "published", "by default", Role::Quiet))
        .row(row("page.title", "“A walk in the rain”", "from the heading of posts/a.md, bytes 0..22", Role::Quiet))
        .row(row("page.slug", "`walk`", "pinned in ink.toml, bytes 40..46", Role::Id))
        .flex(2, Cut::End);
    Trees(vec![source, page])
}

/// A diagnostic with a snippet of the readme fixture, one without a span, and a notice.
struct Reports;

impl Reports {
    fn block(shot: &Shot) -> String {
        let dir = Path::new("fixture/site/readme");
        let loaded = Source::new(dir, &Origin::default()).load().unwrap();
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let ambiguous = Problem::LinkTargetAmbiguous {
            written: "[[Note]]".to_owned(),
            candidates: vec![path("a/Note.md"), path("b/Note.md")],
        };
        let cause = Diagnostic::new(Problem::SourceFileUnreadable { path: path("b/Note.md"), reason: "denied".into() });
        let spanned = Diagnostic::new(ambiguous.clone())
            .on(Subject::File(path("usage.md")))
            .at(Span::new(0, 7).unwrap())
            .caused_by(cause);
        let bare = Diagnostic::new(ambiguous).on(Subject::File(path("usage.md")));
        let notice = Notice::new(Announcement::SiteHomeGenerated).on(Subject::Site);
        [
            shot.text(&Shown { diagnostic: &spanned, texts }),
            shot.text(&Shown { diagnostic: &bare, texts: Texts::NONE }),
            shot.text(&Noted(&notice)),
        ]
        .concat()
    }
}

/// An error of each kind: with a cause, with a partial state, and a usage error.
struct Stopped;

impl Stopped {
    fn errors() -> Vec<Error> {
        let broken = serde_json::from_str::<()>("{ \"files\": [").unwrap_err();
        vec![
            Error::Manifest { path: PathBuf::from(".ink/out/.ink-manifest.json"), source: broken },
            Error::Write { path: PathBuf::from(".ink/out/index.html"), source: std::io::Error::other("no space left") },
            Error::Code { source: "page.titel".parse::<model::AnyCode>().unwrap_err() },
            Refused::Unlinked { dir: PathBuf::from("/home/me/garden") }.into(),
            Refused::Uploading { sent: 212, source: Box::new(Refused::Timeout { after: Duration::from_secs(30) }) }
                .into(),
            Refused::Ended {
                build: "4mq8c1d0wz5e".to_owned(),
                state: JobState::Refused,
                count: 2,
                first: Some(Logged("refused: {\"code\":\"page.slug.clash\"}".to_owned())),
            }
            .into(),
            Refused::Occupied { path: PathBuf::from("out") }.into(),
        ]
    }

    fn failures() -> Vec<Failure> {
        let usage = Cli::try_parse_from(["ink", "explain", "--wy"]).unwrap_err();
        let mut failures: Vec<Failure> =
            Self::errors().iter().map(|error| Failure::of(error, crate::output::exit::Exit::Usage)).collect();
        failures.push(Failure::usage(&usage));
        failures
    }
}

/// Every block, as `(name, text)`.
struct Cases;

impl Cases {
    /// One stage line of each look: plain, closing with each severity, and timed.
    fn stages() -> [StageLine; 7] {
        let tally = |errors, warnings| Tally { errors, warnings };
        let took = Elapsed(Duration::from_millis(1400));
        [
            StageLine::new(Verb::Read, "214 files from git 9f3c2a1b4d7e"),
            StageLine::new(Verb::Understood, "61 pages, 312 decisions"),
            StageLine::new(Verb::Rendered, "61 pages").took(took),
            StageLine::new(Verb::Changes, "2 newly public, 1 moved, against deploy 4mq8c1d0wz5e"),
            StageLine::new(Verb::Built, "187 files in .ink/out, 1 warning").closing(tally(0, 1)).took(took),
            StageLine::new(Verb::Checked, "61 pages, 312 decisions: 1 error, 2 warnings").closing(tally(1, 2)),
            StageLine::new(Verb::WouldPin, "page.slug of posts/a.md; ink.toml is unchanged"),
        ]
    }

    fn all() -> Vec<(String, String)> {
        let mut cases = Vec::new();
        for shot in Shot::all() {
            let stage = Self::stages().iter().map(|line| shot.text(line)).collect::<String>();
            cases.push((format!("stage-{}", shot.name), stage));

            let tick = Tick {
                phase: Phase::Rendering,
                done: 41,
                total: 118,
                unit: "images",
                item: Some("photos/2026/trip-04.jpg".to_owned()),
                elapsed: Duration::from_secs(6),
                bar: 24,
            };
            let meter = shot.text(&tick.live()) + &shot.text(&tick.heartbeat());
            cases.push((format!("meter-{}", shot.name), meter));

            cases.push((format!("diagnostic-{}", shot.name), Reports::block(&shot)));
            let errors: String = Stopped::failures().iter().map(|failure| shot.text(failure)).collect();
            cases.push((format!("error-{}", shot.name), errors));
            cases.push((format!("tree-{}", shot.name), shot.text(&decisions())));

            let published = |glyph: Glyph, doc: &Doc, path: &str, why: &str| {
                vec![doc.glyph(glyph), Line::plain(path), Line::new().with(Role::Quiet, why)]
            };
            let doc = Doc::new(shot.look);
            let table = Table::new(Line::new().with(Role::Heading, "published (2 of 4)"))
                .indent(2)
                .flex(2, Cut::End)
                .row(published(Glyph::Live, &doc, "notes/trip.md", "publish: true in the frontmatter, line 2"))
                .row(published(Glyph::Live, &doc, "notes/trip/day-1.md", "matched by `notes/trip/**` in ink.toml"))
                .row(published(Glyph::Private, &doc, "journal/2026-01-04.md", "nothing selected it"));
            cases.push((format!("table-{}", shot.name), shot.text(&table)));

            let rung = |glyph: Glyph, at: &str, witness: &str, value: &str, verdict: (&str, Role)| {
                vec![
                    doc.glyph(glyph).pad(1).with(Role::Plain, at),
                    Line::plain(witness),
                    Line::plain(value),
                    Line::new().with(verdict.1, verdict.0),
                ]
            };
            let rail = Rail {
                top: Line::plain("A walk in the rain, from the heading of posts/a.md: rung 3 of 5"),
                rows: vec![
                    vec![
                        doc.glyph(Glyph::Gone).pad(1).with(Role::Plain, "1"),
                        Line::plain("pin"),
                        Line::new().with(Role::Quiet, "nothing"),
                    ],
                    rung(Glyph::Done, "3", "heading", "A walk in the rain", ("chosen", Role::Good)),
                    rung(Glyph::Outranked, "4", "filename", "walk", ("outranked", Role::Quiet)),
                ],
                foot: Line::new().with(Role::Quiet, "to pin it: ").with(Role::Literal, "ink pin posts/a.md page.title"),
                flex: None,
            };
            cases.push((format!("rail-{}", shot.name), shot.text(&rail)));

            let record = Record::new(2)
                .field("api", Line::plain("http://api.localhost/v1/sites (host api.localhost)"))
                .field("owner token", Line::plain("in .data/owner-token"))
                .field("limits", Line::plain("json body 65536 B, upload 268435456 B"));
            let standing = Self::standing();
            let linked = Linked {
                schema: Linked::SCHEMA,
                site: "5c0e1f7a9b2d4863a1f0e7c9d2b4a688".to_owned(),
                handle: "garden".to_owned(),
                url: Some("https://garden.uses.ink".to_owned()),
                api: "https://api.uses.ink".to_owned(),
                file: PathBuf::from(".uses/site"),
                created: true,
                unchanged: false,
            };
            let exported = Exported {
                schema: Exported::SCHEMA,
                what: "site",
                dir: PathBuf::from("out"),
                files: 61,
                bytes: 1_204_331,
            };
            let publish =
                shot.text(&Standing::of(&standing)) + &shot.text(&linked) + &shot.text(&Answer::of(exported.said()));
            cases.push((format!("publish-{}", shot.name), publish));

            cases.push((format!("record-{}", shot.name), shot.text(&record) + &shot.text(&Answer::of("5kq3v9m2tj7r"))));
        }
        cases.push(("roles".to_owned(), Self::roles()));
        cases.push(("glyphs".to_owned(), Self::glyphs()));
        cases.push(("json".to_owned(), Self::json()));
        cases
    }

    /// A site that is live, paused, with a build running.
    fn standing() -> Overview {
        serde_json::from_value(serde_json::json!({
            "site": {
                "id": "5c0e1f7a9b2d4863a1f0e7c9d2b4a688", "handle": "garden", "url": "https://garden.uses.ink",
                "state": "active", "created": "2026-11-02T09:00:00Z", "source": { "kind": "upload" }
            },
            "live": {
                "deploy": "4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d", "previous": null, "paused": true,
                "since": "2026-11-20T10:30:00Z"
            },
            "build": {
                "id": "7a1f3c9e5b2d48a6c0e1f7b9d3a5c2e4", "trigger": "upload", "state": "running", "requested": "2026-11-27T08:00:00Z",
                "finished": null, "deploy": null
            },
            "pending": [{
                "id": "7a1f3c9e5b2d48a6c0e1f7b9d3a5c2e4", "trigger": "upload", "state": "running", "requested": "2026-11-27T08:00:00Z",
                "finished": null, "deploy": null
            }]
        }))
        .unwrap()
    }

    /// Each role as the ANSI theme styles it, and again in bold.
    fn roles() -> String {
        let shot = Shot { name: "colour", look: Look { colour: true, ..Look::PLAIN } };
        let mut doc = Doc::new(shot.look);
        for role in Theme::ROLES {
            doc.push(Line::new().with(role, format!("{role:?}")).pad(1).strong(role, format!("{role:?}")));
        }
        shot.text(&Block(doc))
    }

    /// Every glyph in both sets.
    fn glyphs() -> String {
        [
            Glyph::Branch,
            Glyph::Last,
            Glyph::Rail,
            Glyph::Top,
            Glyph::Live,
            Glyph::Done,
            Glyph::Gone,
            Glyph::Private,
            Glyph::Outranked,
            Glyph::Added,
            Glyph::Removed,
            Glyph::Changed,
            Glyph::To,
            Glyph::Truncation,
            Glyph::BarFull,
            Glyph::BarHead,
            Glyph::BarRest,
        ]
        .into_iter()
        .map(|glyph| {
            let (unicode, ascii) = (Glyphs::Unicode.get(glyph), Glyphs::Ascii.get(glyph));
            [format!("{glyph:?}"), unicode.to_owned(), ascii.to_owned(), format!("{:?}\n", glyph.role())].join(" ")
        })
        .collect()
    }

    /// The JSON of every block that has a form of its own.
    fn json() -> String {
        let mut out = String::new();
        for failure in Stopped::failures() {
            out.push_str(&serde_json::to_string_pretty(&failure).unwrap());
            out.push('\n');
        }
        let dir = Path::new("fixture/site/readme");
        let loaded = Source::new(dir, &Origin::default()).load().unwrap();
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let problem = Problem::SourceFileUnreadable { path: path("b/Note.md"), reason: "denied".into() };
        let diagnostic = Diagnostic::new(problem).on(Subject::File(path("usage.md"))).at(Span::new(0, 7).unwrap());
        let notice = Notice::new(Announcement::SiteHomeGenerated).on(Subject::Site);
        for event in [Event::diagnostic(&diagnostic, texts), Event::notice(&notice)] {
            out.push_str(&serde_json::to_string_pretty(&event).unwrap());
            out.push('\n');
        }
        let overview = Self::standing();
        out.push_str(&serde_json::to_string_pretty(&Standing::of(&overview)).unwrap());
        out.push('\n');
        let beat = StageLine::new(Verb::Read, "214 files from git 9f3c2a1b4d7e").beat().unwrap();
        out.push_str(&serde_json::to_string_pretty(&beat).unwrap());
        out.push('\n');
        out
    }
}

/// A finished document, to write as it is.
struct Block(Doc);

impl Render for Block {
    fn render(&self, into: &mut Doc) {
        for line in self.0.lines() {
            into.push(line.clone());
        }
    }
}

#[test]
fn every_block_looks_as_expected() {
    fixture::cases("fixture/term", Cases::all());
}

#[test]
fn the_text_of_an_error_holds_every_field_of_its_json() {
    for failure in Stopped::failures() {
        let json = serde_json::to_value(&failure).unwrap();
        let text = Shot::all()[0].text(&failure);
        assert!(text.contains(json["error"].as_str().unwrap()), "{text}");
        assert!(text.contains(json["what"].as_str().unwrap()), "{text}");
        for cause in json["causes"].as_array().unwrap() {
            assert!(text.contains(cause.as_str().unwrap()), "{text}");
        }
        if let Some(fix) = json["fix"].as_str() {
            assert!(text.contains(fix), "{text}");
        }
        let state = match json["state"]["kind"].as_str().unwrap() {
            "unchanged" => "nothing was changed",
            "partial" => "may be partly written",
            "uploaded" => "run it again to send the rest",
            _ => "is not known",
        };
        assert!(text.contains(state), "{state} in {text}");
    }
}

#[test]
fn the_text_of_an_event_holds_every_field_of_its_json() {
    let dir = Path::new("fixture/site/readme");
    let loaded = Source::new(dir, &Origin::default()).load().unwrap();
    let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
    let ambiguous = Problem::LinkTargetAmbiguous {
        written: "[[Note]]".to_owned(),
        candidates: vec![path("a/Note.md"), path("b/Note.md")],
    };
    let diagnostic = Diagnostic::new(ambiguous).on(Subject::File(path("usage.md"))).at(Span::new(0, 7).unwrap());
    let event = serde_json::to_value(Event::diagnostic(&diagnostic, texts)).unwrap();
    let text = Shot::all()[0].text(&Shown { diagnostic: &diagnostic, texts });
    for field in ["code", "what", "file", "fix"] {
        assert!(text.contains(event[field].as_str().unwrap()), "{field} of {event} in {text}");
    }
    assert!(text.contains(&format!("{}:{}", event["file"].as_str().unwrap(), event["line"])), "{text}");
}
