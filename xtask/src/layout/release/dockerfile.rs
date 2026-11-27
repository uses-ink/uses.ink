//! A Dockerfile as instructions: the keyword and the rest of each line.

#[derive(Debug)]
pub(super) struct Dockerfile {
    lines: Vec<(String, String)>,
}

impl Dockerfile {
    /// Reads `text`, joining `\` continuations and skipping comments.
    pub(super) fn new(text: &str) -> Self {
        let joined = text.replace("\\\n", " ");
        let lines = joined
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| match line.split_once(char::is_whitespace) {
                Some((keyword, rest)) => (keyword.to_uppercase(), rest.trim().to_owned()),
                None => (line.to_uppercase(), String::new()),
            })
            .collect();
        Self { lines }
    }

    /// The rest of every line that starts with `keyword`.
    pub(super) fn keyword<'a>(&'a self, keyword: &'a str) -> impl Iterator<Item = &'a str> {
        self.lines.iter().filter(move |(k, _)| k == keyword).map(|(_, rest)| rest.as_str())
    }

    /// The word after `flag` in any line.
    pub(super) fn after(&self, flag: &str) -> Option<String> {
        let words = self.lines.iter().flat_map(|(_, rest)| rest.split_whitespace());
        words.clone().zip(words.skip(1)).find(|(word, _)| *word == flag).map(|(_, next)| next.to_owned())
    }

    /// The last `keyword` line read as an exec-form array, `["a", "b"]`.
    pub(super) fn exec(&self, keyword: &str) -> Vec<String> {
        let Some(rest) = self.keyword(keyword).last() else { return Vec::new() };
        let inner = rest.trim().trim_start_matches('[').trim_end_matches(']');
        inner.split(',').map(|word| word.trim().trim_matches('"').to_owned()).filter(|w| !w.is_empty()).collect()
    }
}
