//! The build screen's pages.

use super::{Fixtures, Pages};
use build::{Event, Status as Ended, Trigger};
use model::Bytes;
use std::time::Duration;
use uses_ink_web::Failure;
use uses_ink_web::view::{Build, Ended as How, Outcome};

impl Fixtures {
    pub(crate) fn events() -> Vec<Event> {
        vec![
            Event::Started { build: Self::build(1), trigger: Trigger::Push },
            Event::Decided(Self::decisions().remove(0)),
            Event::Diagnosed(Self::problem()),
            Event::Rendered { files: 23, bytes: Bytes(412_000) },
            Event::Published {
                deploy: Self::deploy('a'),
                previous: Some(Self::deploy('b')),
                visible_within: Duration::from_secs(30),
            },
            Event::Finished { status: Ended::Succeeded, wall: Duration::from_millis(1840) },
        ]
    }

    pub(crate) fn refused() -> Vec<Event> {
        vec![
            Event::Started { build: Self::build(2), trigger: Trigger::Manual },
            Event::Refused(Self::problem()),
            Event::Finished { status: Ended::Refused, wall: Duration::from_millis(210) },
        ]
    }

    pub(crate) fn build_pages(pages: &mut Pages) {
        let handle = Self::handle();
        pages.put(
            "build_done",
            &Build::new(
                handle.clone(),
                Self::build(1),
                Self::events(),
                How::Finished(Outcome {
                    status: Ended::Succeeded,
                    deploy: Some(Self::deploy('a')),
                    wall: Duration::from_millis(1840),
                }),
            ),
        );
        pages.put(
            "build_refused",
            &Build::new(
                handle.clone(),
                Self::build(2),
                Self::refused(),
                How::Finished(Outcome { status: Ended::Refused, deploy: None, wall: Duration::from_millis(210) }),
            ),
        );
        pages.put(
            "build_empty",
            &Build::new(handle.clone(), Self::build(4), vec![], How::Cut(Failure::Cut { build: Self::build(4) })),
        );
    }
}
