//! The span timing summary: for each path in the span tree, how often it ran, how long it took
//! in all, how much of that was its own and not its children's, and the slowest run.

use std::fmt::Write as _;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

/// One path of the span tree, as `command[build]` then `stage[render]` and so on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    /// The labels from the outermost span to this one.
    pub path: Vec<String>,
    /// How many times it ran.
    pub count: u64,
    /// The time from opening to closing, summed.
    pub total: Duration,
    /// The time not spent in a child span, summed.
    pub own: Duration,
    /// The longest single run.
    pub slowest: Duration,
}

/// The rows, in the order their paths first opened, so a parent precedes its children.
#[derive(Clone, Debug, Default)]
pub struct Timing(Arc<Mutex<Vec<Row>>>);

impl Timing {
    fn rows_mut<T>(&self, work: impl FnOnce(&mut Vec<Row>) -> T) -> T {
        work(&mut self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }

    pub(crate) fn open(&self, path: &[String]) {
        self.rows_mut(|rows| {
            if !rows.iter().any(|row| row.path == path) {
                rows.push(Row { path: path.to_vec(), ..Row::default() });
            }
        });
    }

    pub(crate) fn close(&self, path: &[String], elapsed: Duration, own: Duration) {
        self.rows_mut(|rows| {
            if let Some(row) = rows.iter_mut().find(|row| row.path == path) {
                row.count += 1;
                row.total += elapsed;
                row.own += own;
                row.slowest = row.slowest.max(elapsed);
            }
        });
    }

    /// The rows so far.
    pub fn rows(&self) -> Vec<Row> {
        self.rows_mut(|rows| rows.clone())
    }

    /// The summary as a table, one row per path, indented by depth.
    pub fn table(&self) -> String {
        let rows = self.rows();
        let width = rows.iter().map(|row| row.path.len() * 2 + row.path.last().map_or(0, String::len)).max().unwrap_or(4);
        let mut text = format!("{:<width$} {:>6} {:>10} {:>10} {:>10}\n", "span", "count", "total", "own", "slowest");
        for row in &rows {
            let name = format!("{}{}", "  ".repeat(row.path.len().saturating_sub(1)), row.path.last().map_or("", String::as_str));
            let _written = writeln!(
                text,
                "{name:<width$} {:>6} {:>10} {:>10} {:>10}",
                row.count,
                format!("{:.1?}", row.total),
                format!("{:.1?}", row.own),
                format!("{:.1?}", row.slowest),
            );
        }
        text
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_parent_precedes_its_children_and_own_time_excludes_them() {
        let timing = Timing::default();
        let root = vec!["command[build]".to_owned()];
        let child = vec!["command[build]".to_owned(), "stage[render]".to_owned()];
        timing.open(&root);
        timing.open(&child);
        timing.close(&child, Duration::from_millis(3), Duration::from_millis(3));
        timing.close(&root, Duration::from_millis(5), Duration::from_millis(2));
        let rows = timing.rows();
        assert_eq!(rows.iter().map(|row| row.path.len()).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(rows[0].own, Duration::from_millis(2));
        assert!(timing.table().contains("  stage[render]"));
    }
}
