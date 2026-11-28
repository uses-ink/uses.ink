//! The deploys screen's pages.

use super::{Fixtures, Pages};
use control::JobState;
use uses_ink_web::view::{Deploys, Serving};

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
        pages.put("deploys_empty", &Deploys::new(handle.clone(), Serving::Nothing, vec![]));
    }
}
