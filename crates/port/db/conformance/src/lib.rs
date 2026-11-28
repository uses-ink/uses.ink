//! The conformance suite of the row port: each case is a type with a `check` over any [`Db`],
//! and [`suite!`] turns every case into a test, so an adapter runs the same ones.
//!
//! ```ignore
//! db_conformance::suite!(|| MyAdapter::memory().unwrap());
//! ```
//!
//! The cases pin what D1 does, since D1 is the stricter target (decision 0030): statements and
//! batches only, foreign keys enforced, a bound-parameter limit, and the migration ledger.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::assert_is_empty,
    clippy::missing_panics_doc,
    reason = "a conformance case stops at the first surprise"
)]

use db::{Constraint, Db, Error, Ledger, Migration, Statement, Steps, Unapplied, Unrenderable, Value};
use sea_query::{ColumnDef, Expr, ExprTrait, ForeignKey, IdenStatic, Order, Query, Table};

#[doc(hidden)]
pub use pollster;

#[derive(Clone, Copy, IdenStatic)]
enum Parent {
    Table,
    Id,
    Name,
    Size,
}

#[derive(Clone, Copy, IdenStatic)]
enum Child {
    Table,
    Id,
    Parent,
}

#[derive(Clone, Copy, IdenStatic)]
enum Every {
    Table,
    A,
    B,
    C,
    D,
    E,
}

#[derive(Clone, Copy, IdenStatic)]
enum Big {
    Table,
    Body,
}

#[derive(Clone, Copy, IdenStatic)]
enum Answer {
    One,
    Two,
    Three,
    Four,
    N,
}

/// A table no case makes.
#[derive(Clone, Copy, IdenStatic)]
enum Nowhere {
    Table,
    Id,
}

#[derive(Clone, Copy, IdenStatic)]
#[iden = "sqlite_master"]
enum Master {
    Table,
    Name,
}

/// The tables and statements the cases share.
struct Tables;

impl Tables {
    /// The two tables: a parent with a unique name and a checked size, and a child that points at it.
    fn create() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Parent::Table)
                .col(ColumnDef::new(Parent::Id).integer().primary_key())
                .col(ColumnDef::new(Parent::Name).text().not_null().unique_key())
                .col(ColumnDef::new(Parent::Size).integer().check(Expr::col(Parent::Size).gte(0)))
                .into(),
            Table::create()
                .table(Child::Table)
                .col(ColumnDef::new(Child::Id).integer().primary_key())
                .col(ColumnDef::new(Child::Parent).integer().not_null())
                .foreign_key(ForeignKey::create().from(Child::Table, Child::Parent).to(Parent::Table, Parent::Id))
                .into(),
        ]
    }

    /// Makes the tables.
    async fn make<D: Db>(db: &D) {
        db.batch(Self::create()).await.unwrap();
    }

    /// Adds a parent.
    fn parent(id: i64, name: Option<&str>, size: i64) -> Statement {
        Query::insert()
            .into_table(Parent::Table)
            .columns([Parent::Id, Parent::Name, Parent::Size])
            .values_panic([Expr::val(id), Expr::val(name), Expr::val(size)])
            .into()
    }

    /// Adds a child of `parent`.
    fn child(id: i64, parent: i64) -> Statement {
        Query::insert()
            .into_table(Child::Table)
            .columns([Child::Id, Child::Parent])
            .values_panic([Expr::val(id), Expr::val(parent)])
            .into()
    }

    /// Every parent's id.
    fn ids() -> Statement {
        Query::select().column(Parent::Id).from(Parent::Table).order_by(Parent::Id, Order::Asc).into()
    }

    /// A select of `count` bound values, the first named `n`.
    fn bound(count: usize) -> Statement {
        let mut select = Query::select();
        select.expr_as(Expr::val(0_i64), Answer::N);
        for n in 1..count {
            select.expr(Expr::val(i64::try_from(n).unwrap()));
        }
        select.into()
    }
}

/// Rows written come back as the same typed values.
#[derive(Debug, Clone, Copy)]
pub struct Values;

impl Values {
    /// An insert of one row of the five columns.
    fn row(values: [Expr; 5]) -> Statement {
        Query::insert()
            .into_table(Every::Table)
            .columns([Every::A, Every::B, Every::C, Every::D, Every::E])
            .values_panic(values)
            .into()
    }

    /// Every kind of value, and the extremes of each, survive a round trip.
    pub async fn check<D: Db>(db: &D) {
        db.run(
            Table::create()
                .table(Every::Table)
                .col(ColumnDef::new(Every::A))
                .col(ColumnDef::new(Every::B))
                .col(ColumnDef::new(Every::C))
                .col(ColumnDef::new(Every::D))
                .col(ColumnDef::new(Every::E))
                .into(),
        )
        .await
        .unwrap();
        let blob = vec![0_u8, 255, 1, 0];
        let text = "h\u{e9}llo \u{1f980} \0 end";
        let low =
            [Expr::val(i64::MIN), Expr::val(1.5_f64), Expr::val(text), Expr::val(blob.clone()), Expr::val(None::<i64>)];
        let high =
            [Expr::val(i64::MAX), Expr::val(-0.0_f64), Expr::val(""), Expr::val(Vec::<u8>::new()), Expr::val(true)];
        db.run(Self::row(low)).await.unwrap();
        db.run(Self::row(high)).await.unwrap();
        let read = Query::select()
            .columns([Every::A, Every::B, Every::C, Every::D, Every::E])
            .from(Every::Table)
            .order_by(Every::A, Order::Asc)
            .into();
        let rows = db.rows(read).await.unwrap();
        let [low, high] = rows.as_slice() else { panic!("two rows, found {}", rows.len()) };
        assert_eq!(low.get::<i64>(Every::A).unwrap(), i64::MIN);
        assert_eq!(low.get::<f64>(Every::B).unwrap(), 1.5);
        assert_eq!(low.get::<String>(Every::C).unwrap(), text);
        assert_eq!(low.get::<Vec<u8>>(Every::D).unwrap(), blob);
        assert_eq!(low.get::<Option<i64>>(Every::E).unwrap(), None);
        assert_eq!(high.get::<i64>(Every::A).unwrap(), i64::MAX);
        assert_eq!(high.get::<String>(Every::C).unwrap(), "");
        assert_eq!(high.get::<Vec<u8>>(Every::D).unwrap(), Vec::<u8>::new());
        assert!(high.get::<bool>(Every::E).unwrap());
        assert_eq!(high.get::<Value>(Every::E).unwrap(), Value::Integer(1));
    }
}

/// A megabyte of bytes goes in and comes out unchanged.
#[derive(Debug, Clone, Copy)]
pub struct Large;

impl Large {
    /// A large blob.
    pub async fn check<D: Db>(db: &D) {
        db.run(Table::create().table(Big::Table).col(ColumnDef::new(Big::Body).blob()).into()).await.unwrap();
        let body: Vec<u8> = (0..1_048_576_u32).map(|n| u8::try_from(n % 251).unwrap()).collect();
        db.run(
            Query::insert().into_table(Big::Table).columns([Big::Body]).values_panic([Expr::val(body.clone())]).into(),
        )
        .await
        .unwrap();
        let row = db.first(Query::select().column(Big::Body).from(Big::Table).into()).await.unwrap().unwrap();
        assert_eq!(row.get::<Vec<u8>>(Big::Body).unwrap(), body);
    }
}

/// Values are data: nothing bound is ever read as SQL.
#[derive(Debug, Clone, Copy)]
pub struct Injection;

impl Injection {
    /// A hostile value is stored and returned as it is, and the table survives.
    pub async fn check<D: Db>(db: &D) {
        Tables::make(db).await;
        let hostile = "x'); DROP TABLE parent; --";
        db.run(Tables::parent(1, Some(hostile), 0)).await.unwrap();
        let find = Query::select()
            .column(Parent::Name)
            .from(Parent::Table)
            .and_where(Expr::col(Parent::Name).eq(hostile))
            .into();
        let row = db.first(find).await.unwrap().unwrap();
        assert_eq!(row.get::<String>(Parent::Name).unwrap(), hostile);
    }
}

/// What a statement reports: the rows it returns and the rows it changes.
#[derive(Debug, Clone, Copy)]
pub struct Outcomes;

impl Outcomes {
    /// Writes count their changes, reads return rows and change none, `RETURNING` does both.
    pub async fn check<D: Db>(db: &D) {
        Tables::make(db).await;
        assert_eq!(db.run(Tables::parent(1, Some("a"), 1)).await.unwrap().changes, 1);
        assert_eq!(db.run(Tables::parent(2, Some("b"), 1)).await.unwrap().changes, 1);
        let grow = Query::update().table(Parent::Table).value(Parent::Size, Expr::col(Parent::Size).add(1)).into();
        let update = db.run(grow).await.unwrap();
        assert_eq!(update.changes, 2);
        assert!(update.rows.is_empty());
        let read = db.run(Tables::ids()).await.unwrap();
        assert_eq!((read.rows.len(), read.changes), (2, 0));
        let take = Query::delete()
            .from_table(Parent::Table)
            .and_where(Expr::col(Parent::Id).eq(2_i64))
            .returning(Query::returning().columns([Parent::Name, Parent::Size]))
            .into();
        let returned = db.run(take).await.unwrap();
        assert_eq!(returned.changes, 1);
        let [row] = returned.rows.as_slice() else { panic!("one returned row") };
        assert_eq!(
            (row.get::<String>(Parent::Name).unwrap(), row.get::<i64>(Parent::Size).unwrap()),
            ("b".to_owned(), 2)
        );
        let none = Query::update()
            .table(Parent::Table)
            .value(Parent::Size, 9_i64)
            .and_where(Expr::col(Parent::Id).eq(99_i64))
            .into();
        assert_eq!(db.run(none).await.unwrap().changes, 0);
    }
}

/// Columns are named as the statement names them.
#[derive(Debug, Clone, Copy)]
pub struct Columns;

impl Columns {
    /// Aliases name columns, a missing column and a wrong type are typed errors.
    pub async fn check<D: Db>(db: &D) {
        let aliased = Query::select()
            .expr_as(Expr::val(1_i64), Answer::One)
            .expr_as(Expr::val("two"), Answer::Two)
            .expr_as(Expr::val(None::<i64>), Answer::Three)
            .into();
        let row = db.first(aliased).await.unwrap().unwrap();
        let names: Vec<&str> = row.columns().map(|(name, _)| name).collect();
        assert_eq!(names, ["one", "two", "three"]);
        assert_eq!(row.get::<Option<String>>(Answer::Three).unwrap(), None);
        assert_eq!(row.get::<i64>(Answer::Four).unwrap_err().column, "four");
        assert!(row.get::<String>(Answer::One).is_err());
        let none = Query::select().expr_as(Expr::val(1_i64), Answer::One).and_where(Expr::val(1_i64).eq(0_i64)).into();
        assert!(db.first(none).await.unwrap().is_none());
    }
}

/// A batch runs in order and returns each outcome.
#[derive(Debug, Clone, Copy)]
pub struct Batches;

impl Batches {
    /// Later statements see earlier ones, and outcomes keep their order.
    pub async fn check<D: Db>(db: &D) {
        Tables::make(db).await;
        let outcomes = db
            .batch(vec![
                Tables::parent(1, Some("a"), 0),
                Query::select().column(Parent::Name).from(Parent::Table).into(),
                Query::update().table(Parent::Table).value(Parent::Size, 5_i64).into(),
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
        Tables::make(db).await;
        db.run(Tables::parent(1, Some("taken"), 0)).await.unwrap();
        let refused =
            db.batch(vec![Tables::parent(2, Some("fresh"), 0), Tables::parent(3, Some("taken"), 0)]).await.unwrap_err();
        assert_eq!(refused.constraint(), Some(Constraint::Unique));
        let left = db.rows(Tables::ids()).await.unwrap();
        assert_eq!(left.len(), 1, "the first statement of a failed batch was applied");
    }
}

/// Constraints are enforced and told apart.
#[derive(Debug, Clone, Copy)]
pub struct Constraints;

impl Constraints {
    /// Unique, foreign key, not null and check each fail as themselves.
    pub async fn check<D: Db>(db: &D) {
        Tables::make(db).await;
        db.run(Tables::parent(1, Some("a"), 0)).await.unwrap();
        let broke = |statement: Statement| async move { db.run(statement).await.unwrap_err().constraint() };
        assert_eq!(broke(Tables::parent(2, Some("a"), 0)).await, Some(Constraint::Unique));
        assert_eq!(broke(Tables::parent(1, Some("z"), 0)).await, Some(Constraint::Unique));
        assert_eq!(broke(Tables::child(1, 99)).await, Some(Constraint::Reference));
        assert_eq!(broke(Tables::parent(3, None, 0)).await, Some(Constraint::Null));
        assert_eq!(broke(Tables::parent(4, Some("n"), -1)).await, Some(Constraint::Check));
        db.run(Tables::child(1, 1)).await.unwrap();
        let orphan = Query::delete().from_table(Parent::Table).and_where(Expr::col(Parent::Id).eq(1_i64)).into();
        assert_eq!(broke(orphan).await, Some(Constraint::Reference));
    }
}

/// Failures that are not a constraint are not reported as one.
#[derive(Debug, Clone, Copy)]
pub struct Failures;

impl Failures {
    /// A table that does not exist is a backend failure.
    pub async fn check<D: Db>(db: &D) {
        let missing = db.run(Query::select().column(Nowhere::Id).from(Nowhere::Table).into()).await.unwrap_err();
        assert!(matches!(missing, Error::Backend { .. }), "{missing}");
        assert_eq!(missing.constraint(), None);
    }
}

/// The bound-parameter limit of D1.
#[derive(Debug, Clone, Copy)]
pub struct Limit;

impl Limit {
    /// 100 parameters run and 101 are refused before they reach the database, in a batch too.
    pub async fn check<D: Db>(db: &D) {
        let ok = Tables::bound(Statement::MAX_PARAMETERS);
        assert_eq!(db.first(ok).await.unwrap().unwrap().get::<i64>(Answer::N).unwrap(), 0);
        let over = Tables::bound(Statement::MAX_PARAMETERS + 1);
        let refused = db.run(over.clone()).await.unwrap_err();
        assert!(
            matches!(refused, Error::Unrenderable { index: 0, source: Unrenderable::TooMany(found) } if found.count == 101 && found.limit == 100)
        );
        let refused = db.batch(vec![Tables::bound(1), over]).await.unwrap_err();
        assert!(matches!(refused, Error::Unrenderable { index: 1, .. }));
    }

    /// An integer that does not fit D1's 64 signed bits is refused, not wrapped.
    pub async fn unsupported<D: Db>(db: &D) {
        let wide = Query::select().expr_as(Expr::val(u64::MAX), Answer::N).into();
        let refused = db.run(wide).await.unwrap_err();
        assert!(matches!(refused, Error::Unrenderable { source: Unrenderable::Value(_), .. }), "{refused}");
    }
}

#[derive(Clone, Copy, IdenStatic)]
enum Alpha {
    Table,
    X,
}

#[derive(Clone, Copy, IdenStatic)]
enum Beta {
    Table,
    Y,
}

/// Makes `alpha` and one row in it.
struct First;

impl Migration for First {
    const NUMBER: u32 = 1;
    const NAME: &'static str = "first";

    fn statements() -> Vec<Statement> {
        vec![
            Table::create().table(Alpha::Table).col(ColumnDef::new(Alpha::X).text()).into(),
            Query::insert().into_table(Alpha::Table).columns([Alpha::X]).values_panic([Expr::val("one")]).into(),
        ]
    }
}

/// Makes `beta`.
struct Second;

impl Migration for Second {
    const NUMBER: u32 = 2;
    const NAME: &'static str = "second";

    fn statements() -> Vec<Statement> {
        vec![Table::create().table(Beta::Table).col(ColumnDef::new(Beta::Y).text()).into()]
    }
}

/// Makes `gamma`, then fails on a table that is not there.
struct Broken;

impl Migration for Broken {
    const NUMBER: u32 = 3;
    const NAME: &'static str = "broken";

    fn statements() -> Vec<Statement> {
        vec![
            Table::create().table(Gamma::Table).col(ColumnDef::new(Gamma::Z).text()).into(),
            Query::insert().into_table(Nowhere::Table).columns([Nowhere::Id]).values_panic([Expr::val(1_i64)]).into(),
        ]
    }
}

#[derive(Clone, Copy, IdenStatic)]
enum Gamma {
    Table,
    Z,
}

/// The migration runner through the port.
#[derive(Debug, Clone, Copy)]
pub struct Migrations;

impl Migrations {
    /// Applies what is missing once, writes wrangler's ledger, and refuses a newer database.
    pub async fn check<D: Db>(db: &D) {
        let first = Steps::new().with::<First>().unwrap();
        let both = Steps::new().with::<First>().unwrap().with::<Second>().unwrap();
        assert_eq!(first.apply(db).await.unwrap().names, ["0001_first.sql"]);
        assert_eq!(both.apply(db).await.unwrap().names, ["0002_second.sql"]);
        assert!(both.apply(db).await.unwrap().names.is_empty());
        let listing = Query::select()
            .columns([Ledger::Id, Ledger::Name, Ledger::AppliedAt])
            .from(Ledger::Table)
            .order_by(Ledger::Id, Order::Asc)
            .into();
        let ledger = db.rows(listing).await.unwrap();
        let names: Vec<String> = ledger.iter().map(|row| row.get(Ledger::Name).unwrap()).collect();
        assert_eq!(names, ["0001_first.sql", "0002_second.sql"]);
        assert!(ledger.iter().all(|row| row.get::<String>(Ledger::AppliedAt).is_ok()));
        let one = db.first(Query::select().column(Alpha::X).from(Alpha::Table).into()).await.unwrap().unwrap();
        assert_eq!(one.get::<String>(Alpha::X).unwrap(), "one");
        // a database ahead of the code is refused, not migrated backwards
        let behind = first.apply(db).await.unwrap_err();
        assert!(matches!(behind, Unapplied::Unknown { ref name } if name == "0002_second.sql"));
    }

    /// A ledger that holds a later migration but not an earlier one is refused, not filled in.
    pub async fn order<D: Db>(db: &D) {
        Steps::new().with::<Second>().unwrap().apply(db).await.unwrap();
        let both = Steps::new().with::<First>().unwrap().with::<Second>().unwrap();
        let skipped = both.apply(db).await.unwrap_err();
        assert!(matches!(
            skipped,
            Unapplied::Order { ref held, ref expected } if held == "0002_second.sql" && expected == "0001_first.sql"
        ));
    }

    /// A step that fails leaves neither its tables nor its ledger row.
    pub async fn broken<D: Db>(db: &D) {
        let steps = Steps::new().with::<First>().unwrap().with::<Second>().unwrap().with::<Broken>().unwrap();
        let failed = steps.apply(db).await.unwrap_err();
        assert!(matches!(failed, Unapplied::Db(_)));
        let listing = Query::select().column(Ledger::Name).from(Ledger::Table).order_by(Ledger::Id, Order::Asc).into();
        let ledger = db.rows(listing).await.unwrap();
        assert_eq!(ledger.len(), 2);
        let table = Query::select()
            .column(Master::Name)
            .from(Master::Table)
            .and_where(Expr::col(Master::Name).eq("gamma"))
            .into();
        assert!(db.first(table).await.unwrap().is_none(), "a failed step left its table");
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
        $crate::suite!(@case $make, refuses_a_value_d1_cannot_carry, Limit, unsupported);
        $crate::suite!(@case $make, applies_migrations_once, Migrations, check);
        $crate::suite!(@case $make, leaves_nothing_of_a_failed_migration, Migrations, broken);
        $crate::suite!(@case $make, refuses_a_ledger_with_a_gap, Migrations, order);
    };
    (@case $make:expr, $name:ident, $case:ident, $method:ident) => {
        #[test]
        fn $name() {
            let (db, _guard) = ($make)();
            $crate::pollster::block_on($crate::$case::$method(&db));
        }
    };
}
