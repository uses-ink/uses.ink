//! The deploys screen's pages.

use super::{Fixtures, Pages};
use control::JobState;
use uses_ink_web::view::{Branch, Deploys, Serving};

impl Fixtures {
    pub(crate) fn deploy_pages(pages: &mut Pages) {
        let handle = Self::handle();
        pages.put(
            "deploys",
            &Deploys::new(
                handle.clone(),
                Serving::Paused(Self::pointer('b')),
                vec![
                    Self::attempt(1, JobState::Succeeded, Some(Self::made('a', false))),
                    Self::attempt(2, JobState::Succeeded, Some(Self::made('b', true))),
                    Self::attempt(3, JobState::Failed, None),
                ],
            ),
        );
        let branch = |label: &str, files, ends| Branch {
            label: label.to_owned(),
            url: format!("https://{label}--ada-notes.uses.ink"),
            files,
            ends: Self::at(ends),
        };
        pages.put(
            "deploys_previews",
            &Deploys::new(handle.clone(), Serving::Live(Self::pointer('b')), vec![]).with_previews(vec![
                branch("new-design", 12, "2026-12-05T09:00:00Z"),
                branch("fix-feed", 9, "2026-12-03T18:30:00Z"),
            ]),
        );
        pages.put("deploys_empty", &Deploys::new(handle.clone(), Serving::Nothing, vec![]));
    }
}
