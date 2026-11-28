//! The settings screen's pages.

use super::{Fixtures, Pages};
use control::Purpose;
use uses_ink_web::view::{Custom, Held, Serving, Settings, Standing};

impl Fixtures {
    pub(crate) fn settings_pages(pages: &mut Pages) {
        let handle = Self::handle();
        let mut settings =
            Settings::new(handle.clone(), Self::address(), Self::source(), Serving::Live(Self::pointer('a')));
        settings.credentials = vec![Held { purpose: Purpose::GitToken, created: Self::at("2026-11-20T08:00:00Z") }];
        settings.domains = vec![
            Custom {
                name: "notes.example.com".to_owned(),
                standing: Standing::Proven(Self::at("2026-11-26T10:00:00Z")),
            },
            Custom {
                name: "www.example.org".to_owned(),
                standing: Standing::Pending {
                    host: "_uses-ink.www.example.org".to_owned(),
                    value: "uses-ink-domain=00112233445566778899aabbccddeeff".to_owned(),
                },
            },
        ];
        pages.put("settings", &settings);
        pages.put("settings_empty", &Settings::new(handle.clone(), Self::address(), Self::source(), Serving::Nothing));
    }
}
