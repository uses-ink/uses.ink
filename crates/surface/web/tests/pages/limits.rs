//! The limits page: public, and made from the values the refusals read.

use std::time::Duration;

use super::{Fixtures, Pages};
use control::{Amount, Budget, Limits, Refusal, Spent};
use model::Bytes;
use uses_ink_web::Document;
use uses_ink_web::view::Limits as Page;

impl Fixtures {
    pub(crate) fn limit_pages(pages: &mut Pages) {
        pages.put("limits", &Page::new(Limits::DEFAULT));
    }
}

#[test]
fn the_page_lists_every_budget_with_its_number() {
    let text = Document(&Page::new(Limits::DEFAULT)).write().unwrap();
    for budget in Budget::ALL {
        assert!(text.contains(budget.name()), "{}", budget.name());
        assert!(text.contains(&Limits::DEFAULT.amount(budget).to_string()), "{}", budget.name());
    }
    assert!(text.contains("1 GiB") && text.contains("60") && text.contains("10 minutes") && text.contains("20"));
}

#[test]
fn a_refusal_and_the_page_read_one_value() {
    let limits = Limits { stored: Bytes(5 * 1024 * 1024), builds: 7, time: Duration::from_secs(90), ..Limits::DEFAULT };
    let text = Document(&Page::new(limits)).write().unwrap();
    for spent in [Spent::Stored, Spent::Builds, Spent::Sites] {
        let Refusal::Over { limit, .. } = limits.over(spent, 1_000_000_000) else { panic!("a budget refusal") };
        let shown = match limits.amount(spent.budget()) {
            Amount::Bytes(bytes) => {
                assert_eq!(bytes.count(), limit);
                bytes.to_string()
            }
            Amount::Count(count) => {
                assert_eq!(count, limit);
                count.to_string()
            }
            _ => panic!("no other amount is a refusal"),
        };
        assert!(text.contains(&shown), "the page shows {shown}, the refusal's limit {limit}");
    }
    assert!(text.contains("90 seconds"));
}
