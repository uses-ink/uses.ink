//! The conformance suite of the row port: each case is a type with a `check` over any [`Db`],
//! and [`suite!`] turns every case into a test, so an adapter runs the same ones.
//!
//! ```ignore
//! db_conformance::suite!(|| MyAdapter::memory().unwrap());
//! ```
//!
//! The cases pin what D1 does, since D1 is the stricter target (decision 0030): statements and
//! batches only, foreign keys enforced, a bound-parameter limit, and the migration ledger.

use db::{Constraint, Db, Error, Statement, Step, Steps, Unapplied, Value};

#[doc(hidden)]
pub use pollster;

/// A table the cases write to.
const PARENT: &str = "CREATE TABLE parent (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, size INTEGER CHECK (size >= 0))";
const CHILD: &str = "CREATE TABLE child (id INTEGER PRIMARY KEY, parent INTEGER NOT NULL REFERENCES parent(id))";

/// Makes the cases' tables.
async fn tables<D: Db>(db: &D) {
    db.batch(vec![Statement::prepare(PARENT), Statement::prepare(CHILD)]).await.unwrap();
}

/// Rows written come back as the same typed values.
#[derive(Debug, Clone, Copy)]
pub struct Values;

impl Values {
    /// Every kind of value, and the extremes of each, survive a round trip.
    pub async fn check<D: Db>(db: &D) {
        db.run(Statement::prepare("CREATE TABLE v (a, b, c, d, e)")).await.unwrap();
        let blob = vec![0_u8, 255, 1, 0];
        db.run(
            Statement::prepare("INSERT INTO v VALUES (?, ?, ?, ?, ?)")
                .bind(i64::MIN)
                .bind(1.5_f64)
                .bind("h\u{e9}llo \u{1f980} \0 end")
                .bind(blob.clone())
                .bind(None::<i64>),
        )
        .await
        .unwrap();
        db.run(Statement::prepare("INSERT INTO v VALUES (?, ?, ?, ?, ?)").bind(i64::MAX).bind(-0.0_f64).bind("").bind(Vec::<u8>::new()).bind(true))
            .await
            .unwrap();
        let rows = db.rows(Statement::prepare("SELECT a, b, c, d, e FROM v ORDER BY a")).await.unwrap();
        let [low, high] = rows.as_slice() else { panic!("two rows, found {}", rows.len()) };
        assert_eq!(low.get::<i64>("a").unwrap(), i64::MIN);
        assert_eq!(low.get::<f64>("b").unwrap(), 1.5);
        assert_eq!(low.get::<String>("c").unwrap(), "h\u{e9}llo \u{1f980} \0 end");
        assert_eq!(low.get::<Vec<u8>>("d").unwrap(), blob);
        assert_eq!(low.get::<Option<i64>>("e").unwrap(), None);
        assert_eq!(high.get::<i64>("a").unwrap(), i64::MAX);
        assert_eq!(high.get::<String>("c").unwrap(), "");
        assert_eq!(high.get::<Vec<u8>>("d").unwrap(), Vec::<u8>::new());
        assert!(high.get::<bool>("e").unwrap());
        assert_eq!(high.get::<Value>("e").unwrap(), Value::Integer(1));
    }
}

/// A megabyte of bytes goes in and comes out unchanged.
#[derive(Debug, Clone, Copy)]
pub struct Large;

impl Large {
    /// A large blob.
    pub async fn check<D: Db>(db: &D) {
        db.run(Statement::prepare("CREATE TABLE big (body BLOB)")).await.unwrap();
        let body: Vec<u8> = (0..1_048_576_u32).map(|n| u8::try_from(n % 251).unwrap()).collect();
        db.run(Statement::prepare("INSERT INTO big VALUES (?)").bind(body.clone())).await.unwrap();
        let row = db.first(Statement::prepare("SELECT body FROM big")).await.unwrap().unwrap();
        assert_eq!(row.get::<Vec<u8>>("body").unwrap(), body);
    }
}

/// Values are data: nothing bound is ever read as SQL.
#[derive(Debug, Clone, Copy)]
pub struct Injection;

impl Injection {
    /// A hostile value is stored and returned as it is, and the table survives.
    pub async fn check<D: Db>(db: &D) {
        tables(db).await;
        let hostile = "x'); DROP TABLE parent; --";
        db.run(Statement::prepare("INSERT INTO parent (id, name, size) VALUES (1, ?, 0)").bind(hostile)).await.unwrap();
        let row = db.first(Statement::prepare("SELECT name FROM parent WHERE name = ?").bind(hostile)).await.unwrap().unwrap();
        assert_eq!(row.get::<String>("name").unwrap(), hostile);
    }
}

/// What a statement reports: the rows it returns and the rows it changes.
#[derive(Debug, Clone, Copy)]
pub struct Outcomes;

impl Outcomes {
    /// Writes count their changes, reads return rows and change none, `RETURNING` does both.
    pub async fn check<D: Db>(db: &D) {
        tables(db).await;
        let insert = |id: i64, name: &str| Statement::prepare("INSERT INTO parent (id, name, size) VALUES (?, ?, 1)").bind(id).bind(name);
        assert_eq!(db.run(insert(1, "a")).await.unwrap().changes, 1);
        assert_eq!(db.run(insert(2, "b")).await.unwrap().changes, 1);
        let update = db.run(Statement::prepare("UPDATE parent SET size = size + 1")).await.unwrap();
        assert_eq!(update.changes, 2);
        assert!(update.rows.is_empty());
        let read = db.run(Statement::prepare("SELECT id FROM parent ORDER BY id")).await.unwrap();
        assert_eq!((read.rows.len(), read.changes), (2, 0));
        let returned = db.run(Statement::prepare("DELETE FROM parent WHERE id = ? RETURNING name, size").bind(2_i64)).await.unwrap();
        assert_eq!(returned.changes, 1);
        let [row] = returned.rows.as_slice() else { panic!("one returned row") };
        assert_eq!((row.get::<String>("name").unwrap(), row.get::<i64>("size").unwrap()), ("b".to_owned(), 2));
        let none = db.run(Statement::prepare("UPDATE parent SET size = 9 WHERE id = ?").bind(99_i64)).await.unwrap();
        assert_eq!(none.changes, 0);
    }
}

/// Columns are named as the statement names them.
#[derive(Debug, Clone, Copy)]
pub struct Columns;

impl Columns {
    /// Aliases name columns, a missing column and a wrong type are typed errors.
    pub async fn check<D: Db>(db: &D) {
        let row = db.first(Statement::prepare("SELECT 1 AS one, 'two' AS two, NULL AS three")).await.unwrap().unwrap();
        let names: Vec<&str> = row.columns().map(|(name, _)| name).collect();
        assert_eq!(names, ["one", "two", "three"]);
        assert_eq!(row.get::<Option<String>>("three").unwrap(), None);
        assert_eq!(row.get::<i64>("four").unwrap_err().column, "four");
        assert!(row.get::<String>("one").is_err());
        assert!(db.first(Statement::prepare("SELECT 1 WHERE 0")).await.unwrap().is_none());
    }
}

/// A batch runs in order and returns each outcome.
#[derive(Debug, Clone, Copy)]
pub struct Batches;

impl Batches {
    /// Later statements see earlier ones, and outcomes keep their order.
    pub async fn check<D: Db>(db: &D) {
        tables(db).await;
        let outcomes = db
            .batch(vec![
                Statement::prepare("INSERT INTO parent (id, name, size) VALUES (1, 'a', 0)"),
                Statement::prepare("SELECT name FROM parent"),
                Statement::prepare("UPDATE parent SET size = 5"),
            ])
            .await
            .unwrap();
        let counts: Vec<(usize, u64)> = outcomes.iter().map(|outcome| (outcome.rows.len(), outcome.changes)).collect();
        assert_eq!(counts, [(0, 1), (1, 0), (0, 1)]);
        assert!(db.batch(Vec::new()).await.unwrap().is_empty());
    }
}

/// A batch is all or nothing.
#[derive(Debug, Clone, Copy)]
pub struct Atomic;

impl Atomic {
    /// A failing statement leaves none of the batch applied, and says which constraint.
    pub async fn check<D: Db>(db: &D) {
        tables(db).await;
        db.run(Statement::prepare("INSERT INTO parent (id, name, size) VALUES (1, 'taken', 0)")).await.unwrap();
        let refused = db
            .batch(vec![
                Statement::prepare("INSERT INTO parent (id, name, size) VALUES (2, 'fresh', 0)"),
                Statement::prepare("INSERT INTO parent (id, name, size) VALUES (3, 'taken', 0)"),
            ])
            .await
            .unwrap_err();
        assert_eq!(refused.constraint(), Some(Constraint::Unique));
        let left = db.rows(Statement::prepare("SELECT id FROM parent")).await.unwrap();
        assert_eq!(left.len(), 1, "the first statement of a failed batch was applied");
    }
}

/// Constraints are enforced and told apart.
#[derive(Debug, Clone, Copy)]
pub struct Constraints;

impl Constraints {
    /// Unique, foreign key, not null and check each fail as themselves.
    pub async fn check<D: Db>(db: &D) {
        tables(db).await;
        db.run(Statement::prepare("INSERT INTO parent (id, name, size) VALUES (1, 'a', 0)")).await.unwrap();
        let broke = |statement: Statement| async move { db.run(statement).await.unwrap_err().constraint() };
        assert_eq!(broke(Statement::prepare("INSERT INTO parent (id, name, size) VALUES (2, 'a', 0)")).await, Some(Constraint::Unique));
        assert_eq!(broke(Statement::prepare("INSERT INTO parent (id, name, size) VALUES (1, 'z', 0)")).await, Some(Constraint::Unique));
        assert_eq!(broke(Statement::prepare("INSERT INTO child (id, parent) VALUES (1, 99)")).await, Some(Constraint::Reference));
        assert_eq!(broke(Statement::prepare("INSERT INTO parent (id, name, size) VALUES (3, NULL, 0)")).await, Some(Constraint::Null));
        assert_eq!(broke(Statement::prepare("INSERT INTO parent (id, name, size) VALUES (4, 'n', -1)")).await, Some(Constraint::Check));
        db.run(Statement::prepare("INSERT INTO child (id, parent) VALUES (1, 1)")).await.unwrap();
        assert_eq!(broke(Statement::prepare("DELETE FROM parent WHERE id = 1")).await, Some(Constraint::Reference));
    }
}

/// Failures that are not a constraint are not reported as one.
#[derive(Debug, Clone, Copy)]
pub struct Failures;

impl Failures {
    /// Bad SQL is a backend failure.
    pub async fn check<D: Db>(db: &D) {
        let error = db.run(Statement::prepare("SELEKT 1")).await.unwrap_err();
        assert!(matches!(error, Error::Backend { .. }), "{error}");
        assert_eq!(error.constraint(), None);
        let missing = db.run(Statement::prepare("SELECT * FROM nowhere")).await.unwrap_err();
        assert!(matches!(missing, Error::Backend { .. }));
    }
}

/// The bound-parameter limit of D1.
#[derive(Debug, Clone, Copy)]
pub struct Limit;

impl Limit {
    /// 100 parameters run and 101 are refused before they reach the database, in a batch too.
    pub async fn check<D: Db>(db: &D) {
        let ok = (0..Statement::MAX_PARAMETERS).fold(Statement::prepare("SELECT ? AS n, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?"), |statement, n| statement.bind(i64::try_from(n).unwrap()));
        assert_eq!(db.first(ok).await.unwrap().unwrap().get::<i64>("n").unwrap(), 0);
        let over = (0..=Statement::MAX_PARAMETERS).fold(Statement::prepare("SELECT ? AS n, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?"), |statement, n| statement.bind(i64::try_from(n).unwrap()));
        let refused = db.run(over.clone()).await.unwrap_err();
        assert!(matches!(refused, Error::Parameters { index: 0, found } if found.count == 101 && found.limit == 100));
        let refused = db.batch(vec![Statement::prepare("SELECT 1"), over]).await.unwrap_err();
        assert!(matches!(refused, Error::Parameters { index: 1, .. }));
    }
}

/// The migration runner through the port.
#[derive(Debug, Clone, Copy)]
pub struct Migrations;

impl Migrations {
    const FIRST: Step = Step::new("0001_first.sql", "CREATE TABLE a (x TEXT); -- one\nINSERT INTO a VALUES ('one');");
    const SECOND: Step = Step::new("0002_second.sql", "CREATE TABLE b (y TEXT);");
    const BROKEN: Step = Step::new("0003_broken.sql", "CREATE TABLE c (z TEXT); INSERT INTO nowhere VALUES (1);");

    /// Applies what is missing once, writes wrangler's ledger, and refuses a newer database.
    pub async fn check<D: Db>(db: &D) {
        let first = Steps::new([Self::FIRST]).unwrap();
        let both = Steps::new([Self::FIRST, Self::SECOND]).unwrap();
        assert_eq!(first.apply(db).await.unwrap().names, ["0001_first.sql"]);
        assert_eq!(both.apply(db).await.unwrap().names, ["0002_second.sql"]);
        assert!(both.apply(db).await.unwrap().names.is_empty());
        let ledger = db.rows(Statement::prepare("SELECT id, name, applied_at FROM d1_migrations ORDER BY id")).await.unwrap();
        let names: Vec<String> = ledger.iter().map(|row| row.get("name").unwrap()).collect();
        assert_eq!(names, ["0001_first.sql", "0002_second.sql"]);
        assert!(ledger.iter().all(|row| row.get::<String>("applied_at").is_ok()));
        let one = db.first(Statement::prepare("SELECT x FROM a")).await.unwrap().unwrap();
        assert_eq!(one.get::<String>("x").unwrap(), "one");
        // a database ahead of the code is refused, not migrated backwards
        let behind = first.apply(db).await.unwrap_err();
        assert!(matches!(behind, Unapplied::Unknown { ref name } if name == "0002_second.sql"));
    }

    /// A step that fails leaves neither its tables nor its ledger row.
    pub async fn broken<D: Db>(db: &D) {
        let steps = Steps::new([Self::FIRST, Self::SECOND, Self::BROKEN]).unwrap();
        let failed = steps.apply(db).await.unwrap_err();
        assert!(matches!(failed, Unapplied::Db(_)));
        let ledger = db.rows(Statement::prepare("SELECT name FROM d1_migrations ORDER BY id")).await.unwrap();
        assert_eq!(ledger.len(), 2);
        let table = db.first(Statement::prepare("SELECT name FROM sqlite_master WHERE name = 'c'")).await.unwrap();
        assert!(table.is_none(), "a failed step left its table");
    }
}

/// Runs every case of the row port's suite against the adapter `$make` builds: a closure that returns the
/// adapter, fresh and empty, with whatever must outlive it (a temporary folder, or `()`) as a
/// pair. Each case is a test of its own, so each has its own budget.
#[macro_export]
macro_rules! suite {
    ($make:expr) => {
        $crate::suite!(@case $make, stores_every_kind_of_value, Values, check);
        $crate::suite!(@case $make, stores_a_large_blob, Large, check);
        $crate::suite!(@case $make, keeps_bound_values_as_data, Injection, check);
        $crate::suite!(@case $make, reports_rows_and_changes, Outcomes, check);
        $crate::suite!(@case $make, names_columns_as_the_statement_does, Columns, check);
        $crate::suite!(@case $make, runs_a_batch_in_order, Batches, check);
        $crate::suite!(@case $make, applies_a_batch_whole_or_not_at_all, Atomic, check);
        $crate::suite!(@case $make, tells_constraints_apart, Constraints, check);
        $crate::suite!(@case $make, does_not_call_a_failure_a_constraint, Failures, check);
        $crate::suite!(@case $make, refuses_more_parameters_than_d1, Limit, check);
        $crate::suite!(@case $make, applies_migrations_once, Migrations, check);
        $crate::suite!(@case $make, leaves_nothing_of_a_failed_migration, Migrations, broken);
    };
    (@case $make:expr, $name:ident, $case:ident, $method:ident) => {
        #[test]
        fn $name() {
            let (db, _guard) = ($make)();
            $crate::pollster::block_on($crate::$case::$method(&db));
        }
    };
}
