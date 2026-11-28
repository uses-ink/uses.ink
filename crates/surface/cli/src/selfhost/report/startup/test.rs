//! The start report: lengths and sizes in the units a person reads, and goldens of the report in
//! plain text, colour, a narrow terminal and JSON, built from one fixed value.

use std::time::Duration;

use clap::Parser as _;

use crate::grammar::args::{Cli, Command, Serve};
use crate::output::term::{Look, Render, Width, Writer};

use super::{Admin, Frame, Hosted, Plan, Release, Source, Span, Stage, Startup, Value};

/// A fixed start: a data folder, one flag changed from its default, and a first-run link.
struct Fixed;

impl Fixed {
    /// The flags of `ink serve` for the fixture.
    fn serve(extra: &[&str]) -> Serve {
        let mut args = vec!["ink", "serve", "--data", "/srv/ink/state", "--addr", "127.0.0.1:8080"];
        args.extend(extra);
        match Cli::try_parse_from(args).unwrap().command {
            Command::Serve(serve) => serve,
            other => panic!("not a serve: {other:?}"),
        }
    }

    /// The report of `serve` as a running start with a first-run link made.
    fn startup(serve: &Serve, stage: Stage) -> Startup {
        let site = serve.origin.clone().unwrap_or_else(|| "http://localhost:8080".to_owned());
        let hosted = Hosted {
            plan: Plan::planned(serve, &site).unwrap(),
            site,
            data: std::path::PathBuf::from("/srv/ink/state"),
            operators: std::path::PathBuf::from("/srv/ink/state/operator-keys"),
            admin: Admin::Issued {
                link: "http://api.localhost:8080/login/Zx3kQ9vT2mLp7RwB4nHd6YcAeF8gJs1U".to_owned(),
                valid: Span(Duration::from_hours(24)),
            },
            retired: false,
        };
        let frame = Frame { serve, address: "127.0.0.1:8080".parse().unwrap(), mode: "single", stage };
        let mut startup = Startup::new(frame, Some(&hosted));
        startup.release = Release { name: "ink", version: "0.0.0" };
        startup
    }

    /// `block` written as `look` writes it, with the escape written `\e` so a diff reads.
    fn text(look: Look, block: &impl Render) -> String {
        let mut out = Vec::new();
        Writer::new(&mut out, look).emit(block).unwrap();
        String::from_utf8(out).unwrap().replace('\x1b', "\\e")
    }
}

#[test]
fn a_length_is_written_in_the_units_a_person_reads() {
    let span = |seconds: u64| Span(Duration::from_secs(seconds)).to_string();
    assert_eq!(span(0), "0s");
    assert_eq!(span(1), "1s");
    assert_eq!(span(90), "1m 30s");
    assert_eq!(span(600), "10m");
    assert_eq!(span(3600), "1h");
    assert_eq!(span(5400), "1h 30m");
    assert_eq!(span(86_400), "1d");
    assert_eq!(span(180 * 86_400), "180d");
    assert_eq!(span(90_061), "1d 1h 1m 1s");
    assert_eq!(Span(Duration::from_millis(250)).to_string(), "250ms");
    assert_eq!(Span(Duration::ZERO).to_string(), "0s");
}

#[test]
fn a_size_is_written_in_binary_units() {
    let size = |bytes: u64| Value::Bytes(bytes).line().text();
    assert_eq!(size(268_435_456), "256 MiB");
    assert_eq!(size(65_536), "64 KiB");
    assert_eq!(size(1_073_741_824), "1 GiB");
    assert_eq!(size(1536), "1.5 KiB");
    assert_eq!(size(0), "0 bytes");
}

#[test]
fn a_flag_that_changed_a_default_is_named_and_the_rest_say_default() {
    let serve = Fixed::serve(&["--upload-bytes", "536870912", "--poll-interval", "30"]);
    let startup = Fixed::startup(&serve, Stage::Serving);
    let rows: Vec<_> = startup.sections.iter().flat_map(|section| section.rows.iter()).collect();
    let source = |key: &str| rows.iter().find(|row| row.key == key).unwrap().source;
    assert_eq!(source("upload_archive"), Source::Flag("--upload-bytes"));
    assert_eq!(source("poll_every"), Source::Flag("--poll-interval"));
    assert_eq!(source("json_body"), Source::Default);
    assert_eq!(source("origin"), Source::Derived("--origin"));
}

#[test]
fn plain_http_is_a_caution_with_its_fix() {
    let startup = Fixed::startup(&Fixed::serve(&[]), Stage::Serving);
    let text = Fixed::text(Look::PLAIN, &startup);
    assert!(text.contains("plain http: the session cookie is not Secure"), "{text}");
    assert!(text.contains("fix: end TLS in a proxy"), "{text}");
}

#[test]
fn the_text_of_the_report_holds_every_row_and_address_of_its_json() {
    let startup = Fixed::startup(&Fixed::serve(&[]), Stage::Serving);
    let json = serde_json::to_value(&startup).unwrap();
    let text = Fixed::text(Look::PLAIN, &startup);
    for address in ["site", "dashboard", "api"] {
        assert!(text.contains(json["urls"][address].as_str().unwrap()), "{address} in {text}");
    }
    assert!(text.contains(json["admin"]["link"].as_str().unwrap()), "{text}");
    for section in json["sections"].as_array().unwrap() {
        for row in section["rows"].as_array().unwrap() {
            let label = row["key"].as_str().unwrap().replace('_', " ");
            assert!(text.contains(&label), "{label} in {text}");
        }
    }
}

#[test]
fn the_first_run_link_is_one_unbroken_token_at_any_width() {
    let startup = Fixed::startup(&Fixed::serve(&[]), Stage::Serving);
    let link = "http://api.localhost:8080/login/Zx3kQ9vT2mLp7RwB4nHd6YcAeF8gJs1U";
    for columns in [40, 60, 80] {
        let look = Look { width: Width::Columns(columns), ..Look::PLAIN };
        assert!(Fixed::text(look, &startup).lines().any(|line| line.contains(link)), "{columns} columns");
    }
}

#[test]
fn the_report_looks_as_expected() {
    let serve =
        Fixed::serve(&["--upload-bytes", "536870912", "--poll-interval", "30", "--mail-from", "ink@example.org"]);
    let startup = Fixed::startup(&serve, Stage::Serving);
    let planned = Fixed::startup(&serve, Stage::Planned);
    let colour = Look { colour: true, ..Look::PLAIN };
    let narrow = Look { width: Width::Columns(60), ..Look::PLAIN };
    let bare = {
        let serve = Fixed::serve(&[]);
        let frame =
            Frame { serve: &serve, address: "127.0.0.1:8080".parse().unwrap(), mode: "single", stage: Stage::Serving };
        let mut startup = Startup::new(frame, None);
        startup.release = Release { name: "ink", version: "0.0.0" };
        startup
    };
    fixture::cases(
        "fixture/serve",
        [
            ("plain", Fixed::text(Look::PLAIN, &startup)),
            ("colour", Fixed::text(colour, &startup)),
            ("narrow", Fixed::text(narrow, &startup)),
            ("planned", Fixed::text(Look::PLAIN, &planned)),
            ("folder", Fixed::text(Look::PLAIN, &bare)),
            ("json", serde_json::to_string_pretty(&startup).unwrap() + "\n"),
        ],
    );
}
