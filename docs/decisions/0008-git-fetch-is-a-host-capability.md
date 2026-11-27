## 0008 `git.fetch` is a host capability

Accepted. Git runs host-side with gix; the git source plugin maps it to a snapshot. **Why:** it is
the one operation that needs a real disk and a native library.
