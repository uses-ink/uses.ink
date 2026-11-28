# Recorded info/refs

`GET <repository>/info/refs?service=git-upload-pack`, as three forges answered it on 2026-11-28 (no
`Git-Protocol` header, so the version 0 advertisement). Each file is the response body with the reference lines
under `refs/pull/` (GitHub, Forgejo), `refs/merge-requests/` and the other GitLab bookkeeping namespaces removed:
every reference line is its own packet, so the rest is byte for byte what the server sent, including the first
line with the capability list.

| file | repository | what it shows |
| --- | --- | --- |
| `github.info-refs` | `https://github.com/octocat/Hello-World.git` | `symref=HEAD:refs/heads/master`, three branches |
| `forgejo.info-refs` | `https://codeberg.org/forgejo/forgejo-docs.git` | `symref=HEAD:refs/heads/next`, 17 branches, a tag |
| `gitlab.info-refs` | `https://gitlab.com/gitlab-org/gitlab-test.git` | 133 branches (some with names that are no branch to this: `'test'`, `100%branch`), six tags, three peeled |
