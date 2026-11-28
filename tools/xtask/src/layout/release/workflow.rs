//! A workflow file, read for the rules every one of them keeps.

use std::path::{Path, PathBuf};

use super::drift::Drift;
use super::pattern::Pattern;

/// A workflow file, read for the rules every one of them keeps.
#[derive(Debug)]
pub(super) struct Workflow {
    pub(super) path: PathBuf,
    pub(super) text: String,
}

impl Workflow {
    /// Every workflow under `.github/workflows`, in name order.
    pub(super) fn all(root: &Path, folder: &str) -> Result<Vec<Self>, Drift> {
        let dir = root.join(folder);
        let unread = |source| Drift::Unreadable { path: folder.into(), source };
        let mut names = std::fs::read_dir(&dir)
            .map_err(unread)?
            .map(|entry| entry.map(|e| e.file_name()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(unread)?;
        names.sort();
        let mut flows = Vec::new();
        for name in names.into_iter().filter(|n| n.to_string_lossy().ends_with(".yml")) {
            let path = PathBuf::from(folder).join(name);
            let text = std::fs::read_to_string(root.join(&path))
                .map_err(|source| Drift::Unreadable { path: path.clone(), source })?;
            flows.push(Self { path, text });
        }
        Ok(flows)
    }

    pub(super) fn drift(&self) -> Vec<Drift> {
        let mut found = self.pins();
        found.extend(self.permits());
        found.extend(self.globs());
        found.extend(self.patterns());
        found.extend(self.shells());
        found.extend(self.tools());
        found
    }

    /// The value of every `key:` line, comment cut off, quotes and list dashes removed.
    fn values<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a str> {
        self.text.lines().filter_map(move |line| {
            let rest = line.trim().trim_start_matches("- ").strip_prefix(key)?;
            Some(rest.split(" #").next().unwrap_or(rest).trim().trim_matches(['"', '\'']))
        })
    }

    /// Every `uses:` that is not a local path or a full commit hash.
    fn pins(&self) -> Vec<Drift> {
        let hashed = |rev: &str| rev.len() == 40 && rev.chars().all(|c| c.is_ascii_hexdigit());
        let unpinned = self.values("uses:").filter(|uses| !uses.starts_with("./"));
        unpinned
            .filter(|uses| !uses.split_once('@').is_some_and(|(_, rev)| hashed(rev)))
            .map(|uses| Drift::Unpinned { path: self.path.clone(), uses: uses.to_owned() })
            .collect()
    }

    /// The top-level `permissions:` block: present, and with no write in it.
    fn permits(&self) -> Option<Drift> {
        let mut lines = self.text.lines().skip_while(|line| !line.starts_with("permissions:"));
        let Some(head) = lines.next() else { return Some(Drift::NoPermissions { path: self.path.clone() }) };
        let body = lines.take_while(|line| line.is_empty() || line.starts_with(char::is_whitespace));
        let wide = std::iter::once(head).chain(body).any(|line| line.contains("write"));
        wide.then(|| Drift::WidePermissions { path: self.path.clone() })
    }

    /// The patterns under `tags:`, comments between them skipped, quotes and list dashes removed.
    fn tags(&self) -> impl Iterator<Item = &str> {
        let after = self.text.lines().map(str::trim).skip_while(|line| *line != "tags:").skip(1);
        let list = after.take_while(|line| line.starts_with("- ") || line.starts_with('#'));
        list.filter_map(|line| line.strip_prefix("- "))
            .map(|item| item.split(" #").next().unwrap_or(item).trim().trim_matches(['"', '\'']))
    }

    /// Tag patterns under `tags:` that hold a `*`.
    fn globs(&self) -> Vec<Drift> {
        self.tags()
            .filter(|pattern| pattern.contains('*'))
            .map(|pattern| Drift::Glob { path: self.path.clone(), pattern: pattern.to_owned() })
            .collect()
    }

    /// Tag patterns under `tags:` that GitHub refuses: it fails the workflow file, and with it
    /// every push, with "push event contained invalid tags patterns".
    fn patterns(&self) -> Vec<Drift> {
        self.tags()
            .filter_map(|pattern| {
                let reject = Pattern::from(pattern).reject()?;
                Some(Drift::TagPattern { path: self.path.clone(), pattern: pattern.to_owned(), reject })
            })
            .collect()
    }

    /// `gh release` lines that carry a `${{ }}` expression.
    fn shells(&self) -> Vec<Drift> {
        let lines = self.text.lines().map(str::trim);
        lines
            .filter(|line| line.contains("gh release") && line.contains("${{"))
            .map(|line| Drift::Interpolated { path: self.path.clone(), line: line.to_owned() })
            .collect()
    }

    /// Tools of the install action that name no version.
    fn tools(&self) -> Vec<Drift> {
        let entries = self.values("tool:").flat_map(|list| list.split(','));
        entries
            .map(str::trim)
            .filter(|tool| !tool.contains('@'))
            .map(|tool| Drift::Floating { path: self.path.clone(), tool: tool.to_owned() })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::Workflow;
    use crate::layout::release::Drift;
    use crate::layout::release::pattern::Reject;

    fn flow(text: &str) -> Workflow {
        Workflow { path: "w.yml".into(), text: text.to_owned() }
    }

    const HASH: &str = "3d3c42e5aac5ba805825da76410c181273ba90b1";

    #[test]
    fn a_moving_tag_is_unpinned() {
        let text = format!("- uses: actions/checkout@v7\n- uses: a/b@{HASH} # v1\n- uses: ./local\n");
        let found = flow(&text).pins();
        assert!(
            matches!(found.as_slice(), [Drift::Unpinned { uses, .. }] if uses == "actions/checkout@v7"),
            "{found:?}"
        );
    }

    #[test]
    fn write_at_the_top_is_wide() {
        assert!(matches!(flow("permissions:\n  contents: write\n").permits(), Some(Drift::WidePermissions { .. })));
        assert!(matches!(flow("name: x\njobs: {}\n").permits(), Some(Drift::NoPermissions { .. })));
        let jobs =
            "permissions:\n  \"contents\": \"read\"\n\njobs:\n  host:\n    permissions:\n      contents: write\n";
        assert!(flow(jobs).permits().is_none());
    }

    #[test]
    fn a_star_in_a_tag_pattern_is_found() {
        let found = flow("on:\n  push:\n    tags:\n      - 'v[0-9]+'\n      - '**[0-9]*'\n").globs();
        assert!(matches!(found.as_slice(), [Drift::Glob { pattern, .. }] if pattern.contains("**")), "{found:?}");
    }

    #[test]
    fn a_comment_between_the_tag_patterns_does_not_end_the_list() {
        let text = "on:\n  push:\n    tags:\n      # why\n      - 'v[0-9]+' # one\n      - 'v*'\n\njobs:\n  - 'x*'\n";
        assert_eq!(flow(text).tags().collect::<Vec<_>>(), ["v[0-9]+", "v*"]);
    }

    #[test]
    fn a_dot_in_a_bracket_of_a_tag_pattern_is_found() {
        let text = "tags:\n  - 'v[0-9]+.[0-9]+'\n  - 'v[0-9]+-[0-9A-Za-z.]+'\n";
        let found = flow(text).patterns();
        assert!(matches!(found.as_slice(), [Drift::TagPattern { reject: Reject::Class('.'), .. }]), "{found:?}");
    }

    #[test]
    fn an_expression_in_a_release_command_is_found() {
        let text = "gh release create \"${{ needs.plan.outputs.tag }}\"\ngh release create \"$TAG\"\n";
        assert_eq!(flow(text).shells().len(), 1);
    }

    #[test]
    fn a_tool_with_no_version_is_floating() {
        let found = flow("  tool: cargo-nextest@0.9.1,wasmtime\n").tools();
        assert!(matches!(found.as_slice(), [Drift::Floating { tool, .. }] if tool == "wasmtime"), "{found:?}");
    }
}
