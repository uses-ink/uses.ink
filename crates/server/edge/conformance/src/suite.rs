//! The native tests of the suite.

/// Makes every case of the suite a native test on a harness `$make` builds fresh, blocking on
/// each with `pollster`, or with `$block` (a function from a future to its output) when the
/// adapter needs a runtime.
#[macro_export]
macro_rules! suite {
    ($make:expr) => {
        $crate::suite!($make, $crate::pollster::block_on);
    };
    ($make:expr, $block:path) => {
        $crate::suite!(@each $make, $block,
            (resolves_a_host_to_its_deploy, Resolves),
            (opens_a_blob_with_its_length, Opens),
            (reads_a_whole_blob_and_a_span, Reads),
            (reads_a_blob_in_its_own_sites_scope_only, Scoped),
            (serves_a_page_and_refuses_a_route_it_lacks, Serves),
            (answers_a_range_with_the_span, Ranges),
            (reads_a_deploy_of_more_than_a_hundred_files, Many),
            (answers_403_and_no_bytes_for_a_suspended_site, Suspends),
            (serves_a_custom_domain_with_its_sites_deploy, Aliases)
        );
    };
    (@each $make:expr, $block:path, $(($name:ident, $case:ident)),*) => {
        $(
            #[test]
            fn $name() {
                let harness = ($make)();
                let outcome = $block($crate::Case::$case.run(&harness));
                if let $crate::Outcome::NotOffered(why) = outcome {
                    eprintln!("not offered: {}: {why}", $crate::Case::$case.name());
                }
            }
        )*
    };
}
