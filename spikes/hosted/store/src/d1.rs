//! D1 values and errors, as the JavaScript boundary gives them.

use worker::js_sys::{Array, ArrayBuffer, BigInt, Object, Uint8Array};
use worker::wasm_bindgen::{JsCast, JsValue};
use worker::{D1Database, Error};

/// The lines the run says.
#[derive(Default)]
pub struct Lines(Vec<String>);

impl Lines {
    /// The body.
    pub fn text(&self) -> String {
        self.0.join("\n") + "\n"
    }

    fn say(&mut self, kind: &str, name: &str, detail: impl std::fmt::Display) {
        self.0.push(format!("{kind} {name}: {detail}"));
    }
}

/// How a JavaScript value looks, for saying what came back.
struct Shown;

impl Shown {
    fn of(value: &JsValue) -> String {
        let kind = value.js_typeof().as_string().unwrap_or_default();
        let text = if value.is_null() {
            "null".to_owned()
        } else if value.is_instance_of::<Array>() || value.is_instance_of::<Uint8Array>() || value.is_instance_of::<ArrayBuffer>() {
            let class = Object::get_prototype_of(value).constructor().name().as_string().unwrap_or_default();
            let json = worker::js_sys::JSON::stringify(value).ok().and_then(|text| text.as_string()).unwrap_or_default();
            let cut: String = json.chars().take(48).collect();
            format!("{class} {cut}")
        } else if let Some(number) = value.as_f64() {
            format!("{number}")
        } else if let Some(text) = value.as_string() {
            format!("{text:?}")
        } else {
            "?".to_owned()
        };
        format!("{kind} {text}")
    }

    fn error(error: &Error) -> String {
        match error {
            Error::D1(d1) => format!("D1 cause: {}", d1.cause()),
            other => format!("{other}"),
        }
    }
}

/// The spike.
pub struct Values {
    db: D1Database,
    lines: Lines,
}

impl Values {
    /// A run on `db`.
    pub fn new(db: D1Database) -> Self {
        Self { db, lines: Lines::default() }
    }

    /// Every check, in order.
    pub async fn run(mut self) -> Lines {
        self.schema().await;
        self.integers().await;
        self.blobs().await;
        self.texts().await;
        self.limits().await;
        self.errors().await;
        self.batches().await;
        self.lines
    }

    /// Runs one statement with its values and returns its rows, or says why it did not.
    async fn rows(&self, sql: &str, values: &[JsValue]) -> Result<Vec<Array>, String> {
        let statement = self.db.prepare(sql).bind(values).map_err(|error| Shown::error(&error))?;
        let rows = statement.raw_js_value().await.map_err(|error| Shown::error(&error))?;
        Ok(rows.into_iter().map(|row| row.unchecked_into::<Array>()).collect())
    }

    async fn schema(&mut self) {
        let made = self
            .db
            .exec("CREATE TABLE ints (id INTEGER PRIMARY KEY, n INTEGER, u, x TEXT);\nCREATE TABLE blobs (id INTEGER PRIMARY KEY, b BLOB, u);\nCREATE TABLE texts (id INTEGER PRIMARY KEY, x TEXT, r REAL, u);\nCREATE TABLE parent (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, size INTEGER CHECK (size >= 0));\nCREATE TABLE child (id INTEGER PRIMARY KEY, parent INTEGER NOT NULL REFERENCES parent(id))")
            .await;
        match made {
            Ok(_) => self.lines.say("ok", "schema", "created with exec"),
            Err(error) => self.lines.say("fail", "schema", Shown::error(&error)),
        }
    }

    /// i64 at and past 2^53, as a number, a bigint and text.
    async fn integers(&mut self) {
        let big = |n: i64| JsValue::from(BigInt::from(n));
        let cases: Vec<(&str, &str, JsValue)> = vec![
            ("number 0", "0", JsValue::from_f64(0.0)),
            ("number 2^53-1", "9007199254740991", JsValue::from_f64(9_007_199_254_740_991.0)),
            ("number -(2^53-1)", "-9007199254740991", JsValue::from_f64(-9_007_199_254_740_991.0)),
            ("number 2^53", "9007199254740992", JsValue::from_f64(9_007_199_254_740_992.0)),
            ("number 2^53+1 as f64", "9007199254740993", JsValue::from_f64(9_007_199_254_740_993_i64 as f64)),
            ("bigint 2^53+1", "9007199254740993", big(9_007_199_254_740_993)),
            ("bigint i64::MAX", "9223372036854775807", big(i64::MAX)),
            ("bigint i64::MIN", "-9223372036854775808", big(i64::MIN)),
            ("text i64::MAX", "9223372036854775807", JsValue::from_str("9223372036854775807")),
            ("text i64::MIN", "-9223372036854775808", JsValue::from_str("-9223372036854775808")),
            ("text 2^53+1", "9007199254740993", JsValue::from_str("9007199254740993")),
            ("text u64::MAX", "18446744073709551615", JsValue::from_str("18446744073709551615")),
        ];
        for (id, (name, decimal, value)) in cases.into_iter().enumerate() {
            let id = JsValue::from_f64(id as f64);
            let insert = self.rows("INSERT INTO ints (id, n, u, x) VALUES (?1, ?2, ?2, ?2)", &[id.clone(), value]).await;
            if let Err(why) = insert {
                self.lines.say("no", name, format!("bind refused: {why}"));
                continue;
            }
            let read = self
                .rows("SELECT typeof(n), n, CAST(n AS TEXT), typeof(u), u, typeof(x), x FROM ints WHERE id = ?1", &[id])
                .await;
            let Ok(rows) = read else {
                self.lines.say("fail", name, "stored but not read");
                continue;
            };
            let Some(row) = rows.first() else {
                self.lines.say("fail", name, "no row");
                continue;
            };
            let cast = row.get(2).as_string().unwrap_or_default();
            let read_n = Shown::of(&row.get(1));
            let exact_stored = cast == decimal;
            let exact_read = row.get(1).as_f64().is_some_and(|n| format!("{n}") == decimal);
            self.lines.say(
                if exact_stored && exact_read { "ok" } else { "no" },
                name,
                format!(
                    "n[{}]: stored as {} (exact {exact_stored}), read as {read_n} (exact {exact_read}); u[{}] {}; x[{}] {}",
                    row.get(0).as_string().unwrap_or_default(),
                    cast,
                    row.get(3).as_string().unwrap_or_default(),
                    Shown::of(&row.get(4)),
                    row.get(5).as_string().unwrap_or_default(),
                    Shown::of(&row.get(6)),
                ),
            );
        }
    }

    /// Bytes bound as a typed array, a buffer and an array of numbers; empty is not null; a megabyte.
    async fn blobs(&mut self) {
        let bytes = [0_u8, 255, 1, 0];
        let typed = Uint8Array::from(&bytes[..]);
        let buffer = typed.buffer();
        let numbers: Array = bytes.iter().map(|byte| JsValue::from_f64(f64::from(*byte))).collect();
        let empty = Uint8Array::new_with_length(0);
        let cases: Vec<(&str, JsValue)> = vec![
            ("uint8array", typed.into()),
            ("arraybuffer", buffer.into()),
            ("array of numbers", numbers.into()),
            ("empty uint8array", empty.into()),
            ("null", JsValue::NULL),
        ];
        for (id, (name, value)) in cases.into_iter().enumerate() {
            let id = JsValue::from_f64(id as f64);
            if let Err(why) = self.rows("INSERT INTO blobs (id, b, u) VALUES (?1, ?2, ?2)", &[id.clone(), value]).await {
                self.lines.say("no", name, format!("bind refused: {why}"));
                continue;
            }
            match self.rows("SELECT typeof(b), hex(b), b FROM blobs WHERE id = ?1", &[id]).await {
                Ok(rows) => {
                    let row = rows.first().cloned().unwrap_or_default();
                    self.lines.say(
                        "note",
                        name,
                        format!("stored {} hex {}; read as {}", row.get(0).as_string().unwrap_or_default(), row.get(1).as_string().unwrap_or_default(), Shown::of(&row.get(2))),
                    );
                }
                Err(why) => self.lines.say("fail", name, why),
            }
        }
        let body: Vec<u8> = (0..1_048_576_u32).map(|n| u8::try_from(n % 251).unwrap_or(0)).collect();
        let started = worker::js_sys::Date::now();
        let put = self.rows("INSERT INTO blobs (id, b) VALUES (100, ?1)", &[Uint8Array::from(&body[..]).into()]).await;
        let got = self.rows("SELECT b FROM blobs WHERE id = 100", &[]).await;
        let elapsed = worker::js_sys::Date::now() - started;
        match (put, got) {
            (Ok(_), Ok(rows)) => {
                let read = rows.first().map(|row| row.get(0)).unwrap_or(JsValue::NULL);
                let back: Vec<u8> = Array::from(&read).iter().filter_map(|n| n.as_f64()).map(|n| n as u8).collect();
                let kind = if back == body { "ok" } else { "no" };
                self.lines.say(kind, "1 MiB blob", format!("round trip in {elapsed} ms, read back {} bytes as {}", back.len(), Shown::of(&read).chars().take(24).collect::<String>()));
            }
            (put, got) => self.lines.say("no", "1 MiB blob", format!("put {:?}, read {:?}", put.err(), got.err())),
        }
    }

    /// Text with a NUL and an emoji, reals, and the empty string.
    async fn texts(&mut self) {
        let text = "h\u{e9}llo \u{1f980} \0 end";
        let id = |n: f64| JsValue::from_f64(n);
        let put = self
            .rows("INSERT INTO texts (id, x, r, u) VALUES (?1, ?2, ?3, ?3)", &[id(1.0), JsValue::from_str(text), JsValue::from_f64(2.0)])
            .await;
        let _ = put;
        match self.rows("SELECT x, length(x), typeof(r), r, typeof(u), u FROM texts WHERE id = 1", &[]).await {
            Ok(rows) => {
                let row = rows.first().cloned().unwrap_or_default();
                let same = row.get(0).as_string().as_deref() == Some(text);
                self.lines.say(
                    if same { "ok" } else { "no" },
                    "text with NUL and emoji",
                    format!("equal {same}; length {}; real column bound 2.0 reads {} {}; untyped column {} {}", Shown::of(&row.get(1)), row.get(2).as_string().unwrap_or_default(), Shown::of(&row.get(3)), row.get(4).as_string().unwrap_or_default(), Shown::of(&row.get(5))),
                );
            }
            Err(why) => self.lines.say("fail", "text with NUL and emoji", why),
        }
        let _ = self.rows("INSERT INTO texts (id, x, r, u) VALUES (2, '', -0.0, 1.5)", &[]).await;
        if let Ok(rows) = self.rows("SELECT x, r, u, typeof(u) FROM texts WHERE id = 2", &[]).await {
            let row = rows.first().cloned().unwrap_or_default();
            self.lines.say("note", "empty text, -0.0 and 1.5", format!("{} / {} / {} {}", Shown::of(&row.get(0)), Shown::of(&row.get(1)), Shown::of(&row.get(2)), row.get(3).as_string().unwrap_or_default()));
        }
    }

    /// How many bound values a statement takes here.
    async fn limits(&mut self) {
        for count in [100_usize, 101, 500, 1000] {
            let sql = format!("SELECT {}", (1..=count).map(|n| format!("?{n}")).collect::<Vec<_>>().join(", "));
            let values: Vec<JsValue> = (0..count).map(|n| JsValue::from_f64(n as f64)).collect();
            match self.rows(&sql, &values).await {
                Ok(rows) => self.lines.say(if count <= 100 { "ok" } else { "no" }, &format!("{count} bound values"), format!("accepted, {} columns", rows.first().map_or(0, Array::length))),
                Err(why) => self.lines.say(if count <= 100 { "no" } else { "ok" }, &format!("{count} bound values"), format!("refused: {why}")),
            }
        }
        match self.rows("SELECT 1 AS a, 2 AS a, ?1 AS b", &[JsValue::NULL]).await {
            Ok(rows) => self.lines.say("note", "duplicate column names", format!("raw gives {} columns", rows.first().map_or(0, Array::length))),
            Err(why) => self.lines.say("fail", "duplicate column names", why),
        }
    }

    /// What each broken constraint says, so the adapter can tell them apart.
    async fn errors(&mut self) {
        let _ = self.rows("INSERT INTO parent (id, name, size) VALUES (1, 'a', 0)", &[]).await;
        let breaks = [
            ("unique name", "INSERT INTO parent (id, name, size) VALUES (2, 'a', 0)"),
            ("primary key", "INSERT INTO parent (id, name, size) VALUES (1, 'z', 0)"),
            ("foreign key", "INSERT INTO child (id, parent) VALUES (1, 99)"),
            ("not null", "INSERT INTO parent (id, name, size) VALUES (3, NULL, 0)"),
            ("check", "INSERT INTO parent (id, name, size) VALUES (4, 'n', -1)"),
            ("missing table", "SELECT id FROM nowhere"),
            ("syntax", "SELEC 1"),
        ];
        for (name, sql) in breaks {
            match self.rows(sql, &[]).await {
                Ok(_) => self.lines.say("no", name, "was accepted"),
                Err(why) => self.lines.say("note", name, why),
            }
        }
        let _ = self.rows("INSERT INTO child (id, parent) VALUES (1, 1)", &[]).await;
        match self.rows("DELETE FROM parent WHERE id = 1", &[]).await {
            Ok(_) => self.lines.say("no", "delete a referenced parent", "was accepted"),
            Err(why) => self.lines.say("note", "delete a referenced parent", why),
        }
    }

    /// Empty and failing batches, and what `changes` says after a read.
    async fn batches(&mut self) {
        match self.db.batch(Vec::new()).await {
            Ok(results) => self.lines.say("note", "empty batch", format!("accepted, {} results", results.len())),
            Err(error) => self.lines.say("note", "empty batch", Shown::error(&error)),
        }
        let good = self.db.prepare("INSERT INTO parent (id, name, size) VALUES (10, 'fresh', 0)");
        let dup = self.db.prepare("INSERT INTO parent (id, name, size) VALUES (11, 'a', 0)");
        match self.db.batch(vec![good, dup]).await {
            Ok(_) => self.lines.say("no", "failing batch", "was accepted"),
            Err(error) => self.lines.say("note", "failing batch", Shown::error(&error)),
        }
        let left = self.rows("SELECT id FROM parent WHERE id = 10", &[]).await.map(|rows| rows.len());
        self.lines.say(if left == Ok(0) { "ok" } else { "no" }, "failing batch left nothing", format!("{left:?} rows of the first statement"));
        let update = self.db.prepare("UPDATE parent SET size = 5");
        let select = self.db.prepare("SELECT id FROM parent");
        match self.db.batch(vec![update, select]).await {
            Ok(results) => {
                for (position, result) in results.iter().enumerate() {
                    let meta = result.meta().ok().flatten();
                    self.lines.say("note", &format!("batch result {position}"), format!("changes {:?}, rows_read {:?}, rows_written {:?}", meta.as_ref().and_then(|m| m.changes), meta.as_ref().and_then(|m| m.rows_read), meta.as_ref().and_then(|m| m.rows_written)));
                }
            }
            Err(error) => self.lines.say("fail", "batch meta", Shown::error(&error)),
        }
    }
}
