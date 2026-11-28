//! The site and understood screens' pages.

use super::{Fixtures, Pages};
use control::JobState;
use engine::{Standing, Witnessed};
use model::code::Title;
use model::decision::File;
use model::{Decision, Evidence, Span, Subject};
use uses_ink_web::view::{Heard, Rung, Serving, Site, Understood, Why};
use uses_ink_web::{Frame, Only, Screen};

impl Fixtures {
    pub(crate) fn site_pages(pages: &mut Pages) {
        let handle = Self::handle();
        let mut live = Site::new(handle.clone(), Self::address(), Serving::Live(Self::pointer('a')));
        live.recent = vec![
            Self::attempt(1, JobState::Succeeded, Some(Self::made('a', true))),
            Self::attempt(2, JobState::Refused, None),
        ];
        live.observed = model::Observed::new(vec![model::Ability::Snapshot, model::Ability::History], vec![]);
        live.problems = vec![Self::problem()];
        pages.put("site_live", &live);
        let mut paused = Site::new(handle.clone(), Self::address(), Serving::Paused(Self::pointer('b')));
        paused.recent = vec![Self::attempt(3, JobState::Running, None)];
        pages.put("site_paused_building", &paused);
        pages.put("site_empty", &Site::new(handle.clone(), Self::address(), Serving::Nothing));

        let mut understood = Understood::new(handle.clone(), Self::source(), Self::decisions());
        understood.deploy = Some(Self::deploy('a'));
        pages.put("understood", &understood);
        understood.only = Only::Check;
        pages.put("understood_check_only", &understood);
        pages.put("understood_empty", &Understood::new(handle.clone(), Self::source(), vec![]));
        pages.put("why", &Self::why());
        pages.put("why_empty", &Why { rungs: vec![], ..Self::why() });
    }

    pub(crate) fn why() -> Why {
        let post = Self::path("posts/rust-allocators.md");
        let said = |value: &str, evidence: Evidence, chosen: bool| Heard {
            testimony: Decision::<Title>::new(File(post.clone()), value.to_owned(), evidence).into(),
            standing: Standing::Value,
            chosen,
        };
        Why {
            frame: Frame::of(&Self::handle(), Screen::Understood),
            handle: Self::handle(),
            code: model::AnyCode::Title,
            subject: Subject::File(post.clone()),
            rungs: vec![
                Rung { witness: Witnessed::Pin, heard: vec![] },
                Rung { witness: Witnessed::Front, heard: vec![] },
                Rung {
                    witness: Witnessed::Heading,
                    heard: vec![said(
                        "Rust allocators",
                        Evidence::Heading { path: post.clone(), span: Span::new(0, 17).unwrap() },
                        true,
                    )],
                },
                Rung {
                    witness: Witnessed::Default,
                    heard: vec![said("posts", Evidence::Filename { path: post.clone() }, false)],
                },
            ],
        }
    }
}
