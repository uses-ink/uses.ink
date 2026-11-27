//! The settings screen's pages.

use super::{Fixtures, Pages};
use control::Purpose;
use uses_ink_web::view::{Held, Serving, Settings};

impl Fixtures {
    pub(crate) fn settings_pages(pages: &mut Pages) {
        let handle = Self::handle();
        let mut settings =
            Settings::new(handle.clone(), Self::address(), Self::source(), Serving::Live(Self::pointer('a')));
        settings.credentials = vec![Held { purpose: Purpose::GitToken, created: Self::at("2026-11-20T08:00:00Z") }];
        pages.put("settings", &settings);
        pages.put("settings_empty", &Settings::new(handle.clone(), Self::address(), Self::source(), Serving::Nothing));
    }
}
