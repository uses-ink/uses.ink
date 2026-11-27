//! The JSON-lines layer: one object per line, typed values stay numbers and flags, an error
//! keeps its causes as an array, and the spans an entry sits in are listed with their fields.

use serde_json::{Map, Number, Value as Json, json};

use crate::Spans;
use crate::layer::{Entry, Frame, Sink, What};
use crate::record::{Fields, Value};

/// A JSON-lines writer.
#[derive(Clone, Debug)]
pub(crate) struct Lines {
    pub(crate) sink: Sink,
    pub(crate) spans: Spans,
}

impl Lines {
    fn value(value: &Value) -> Json {
        match value {
            Value::Unsigned(n) => json!(n),
            Value::Signed(n) => json!(n),
            Value::Float(n) => Number::from_f64(*n).map_or(Json::Null, Json::Number),
            Value::Flag(flag) => json!(flag),
            Value::Text(text) => json!(text),
            Value::Error(chain) => json!({ "message": chain.message, "causes": chain.causes }),
        }
    }

    fn fields(fields: &Fields) -> Json {
        Json::Object(fields.iter().map(|(key, value)| (key.to_owned(), Self::value(value))).collect::<Map<_, _>>())
    }

    fn frames(frames: &[Frame]) -> Json {
        Json::Array(
            frames.iter().map(|frame| json!({ "name": frame.name, "fields": Self::fields(&frame.fields) })).collect(),
        )
    }

    pub(crate) fn write(&self, entry: &Entry<'_>) {
        let micros = |time: &std::time::Duration| json!(u64::try_from(time.as_micros()).unwrap_or(u64::MAX));
        let mut line = Map::new();
        line.insert("t".to_owned(), json!(entry.at.as_secs_f64()));
        line.insert("level".to_owned(), json!(entry.level.as_str()));
        line.insert("target".to_owned(), json!(entry.target));
        // for a span, its own frame is last and the rest are its ancestors
        let (name, ancestors) = match &entry.what {
            What::Open if self.spans.opens() => {
                line.insert("span".to_owned(), json!("open"));
                Self::split(entry.frames)
            }
            What::Close { elapsed, own } if self.spans.closes() => {
                line.insert("span".to_owned(), json!("close"));
                line.insert("elapsed_us".to_owned(), micros(elapsed));
                line.insert("own_us".to_owned(), micros(own));
                Self::split(entry.frames)
            }
            What::Event { name } => ((*name).to_owned(), entry.frames),
            _ => return,
        };
        line.insert("name".to_owned(), json!(name));
        line.insert("fields".to_owned(), Self::fields(entry.fields));
        line.insert("spans".to_owned(), Self::frames(ancestors));
        let mut text = Json::Object(line).to_string();
        text.push('\n');
        self.sink.put(&text);
    }

    fn split(frames: &[Frame]) -> (String, &[Frame]) {
        match frames.split_last() {
            Some((own, ancestors)) => (own.name.to_owned(), ancestors),
            None => (String::new(), frames),
        }
    }
}
