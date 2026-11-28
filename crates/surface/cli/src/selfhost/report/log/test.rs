//! The operator log: goldens of its lines in plain text, colour, a narrow terminal and JSON, and
//! what is shown at which level.

use std::time::Duration;

use edge::Status;
use edge_axum::{Answered, Called, Event as Served, Kind};

use crate::output::term::{Look, Width, Writer};
use crate::selfhost::tasks::{Cause, Event, Task};

use super::{Entry, Happens, Level, Stamp, Zone};

/// Entries at one fixed time.
struct Fixed;

impl Fixed {
    /// The time every entry carries: 2026-11-28 10:30:00 UTC.
    fn stamp() -> Stamp {
        Stamp { at: jiff::Timestamp::from_second(1_795_861_800).unwrap(), zone: Zone::Utc }
    }

    fn answered(status: Status, path: &str) -> Answered {
        Answered {
            method: "GET".to_owned(),
            path: path.to_owned(),
            hostname: None,
            status,
            kind: Kind::Served,
            took: Duration::from_millis(3),
            refusal: None,
            fault: None,
        }
    }

    /// One event of each look: a loop starting, a crash, a give-up with its fix, a failed build
    /// with its causes, a request, an API call and a stop.
    fn events() -> Vec<Box<dyn Happens>> {
        let broken = std::io::Error::other("the builder component is missing");
        vec![
            Box::new(Event::Started(Task::Worker)),
            Box::new(Event::Crashed {
                task: Task::Watch,
                attempt: 2,
                cause: Cause::Panicked("poll failed".to_owned()),
            }),
            Box::new(Event::Restarting { task: Task::Watch, after: Duration::from_secs(2) }),
            Box::new(Event::GaveUp { task: Task::Watch, attempts: 6 }),
            Box::new(Event::Failed { cause: edge_control::Chain::of(&broken) }),
            Box::new(Served::Answered(Self::answered(Status::Ok, "/tea"))),
            Box::new(Served::Answered(Self::answered(Status::NotFound, "/no/such/page"))),
            Box::new(Served::Called(Called {
                method: "POST".to_owned(),
                path: "/v1/sites".to_owned(),
                status: 201,
                took: Duration::from_millis(1400),
            })),
            Box::new(Served::Stopping),
        ]
    }

    fn entries() -> Vec<Entry> {
        Self::events().iter().map(|event| Entry::of(event.as_ref(), Self::stamp())).collect()
    }

    fn text(look: Look) -> String {
        let mut out = Vec::new();
        let mut writer = Writer::new(&mut out, look);
        for entry in Self::entries() {
            writer.emit(&entry).unwrap();
        }
        String::from_utf8(out).unwrap().replace('\x1b', "\\e")
    }
}

#[test]
fn the_log_looks_as_expected() {
    let colour = Look { colour: true, ..Look::PLAIN };
    let narrow = Look { width: Width::Columns(60), ..Look::PLAIN };
    let json: String = Fixed::entries().iter().map(|entry| serde_json::to_string(entry).unwrap() + "\n").collect();
    fixture::cases(
        "fixture/log",
        [
            ("plain", Fixed::text(Look::PLAIN)),
            ("colour", Fixed::text(colour)),
            ("narrow", Fixed::text(narrow)),
            ("json", json),
        ],
    );
}

#[test]
fn an_ordinary_request_is_shown_only_when_asked_and_a_failure_always() {
    let ordinary = Fixed::answered(Status::Ok, "/tea");
    assert!(ordinary.request());
    assert_eq!(ordinary.level(), Level::Info);
    let missing = Fixed::answered(Status::NotFound, "/nothing");
    assert!(missing.request(), "a page that is not there is ordinary");
    let broken = Fixed::answered(Status::Internal, "/tea");
    assert!(!broken.request());
    assert_eq!(broken.level(), Level::Error);
    assert!(!Event::Started(Task::Watch).request());
}

#[test]
fn a_crash_says_its_cause_and_a_give_up_says_what_to_do() {
    let gave = Entry::of(&Event::GaveUp { task: Task::Worker, attempts: 6 }, Fixed::stamp());
    assert_eq!(gave.level, Level::Error);
    assert!(gave.fix.is_some());
    let crashed = Entry::of(&Event::Crashed { task: Task::Worker, attempt: 1, cause: Cause::Returned }, Fixed::stamp());
    assert_eq!(crashed.causes, ["returned"]);
}

#[test]
fn the_time_is_rfc_3339_to_the_second() {
    assert_eq!(Fixed::stamp().to_string(), "2026-11-28T10:30:00Z");
}
