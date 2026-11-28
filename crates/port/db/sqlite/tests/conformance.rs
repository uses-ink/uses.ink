//! The adapter against the port's conformance suite.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_db_sqlite as db_sqlite;

db_conformance::suite!(|| (db_sqlite::Sqlite::memory().unwrap(), ()));

/// The case list, run by an executor that is not the macro's, passes the same cases.
#[test]
fn runs_the_case_list() {
    for case in db_conformance::Case::ALL {
        let db = db_sqlite::Sqlite::memory().unwrap();
        db_conformance::pollster::block_on(case.run(&db));
    }
}
