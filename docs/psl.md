# Public Suffix List submission

`uses.ink` goes on the Public Suffix List so every `<handle>.uses.ink` is its own site to
browsers: no shared cookies, separate storage, separate permission prompts (architecture.md
section 13). Review takes weeks, so it starts before there is anything to host.

The owner sends it; it is a public pull request under their name and a DNS record on the zone.

## Before

- The `uses.ink` registration must have at least two years left; renew first if not.
- Read the current rules: https://github.com/publicsuffix/list/wiki/Guidelines

## Steps

1. Fork `publicsuffix/list` and add, in the PRIVATE DOMAINS section, in alphabetical order by
   the entry's organisation comment:

   ```
   // uses.ink : https://uses.ink
   // Submitted by <name> <address>
   uses.ink
   ```

2. Open the pull request with the template filled in. The reason, in the list's own terms:
   uses.ink hosts sites for unrelated people at `<handle>.uses.ink`; each must be isolated from
   the others for cookies and storage. Subdomains are assigned to users; the apex and `app.`
   are ours.

3. Add the TXT record the maintainers check, holding the pull request's URL:

   ```
   _psl.uses.ink.  TXT  "https://github.com/publicsuffix/list/pull/<number>"
   ```

4. Keep the record for as long as the entry is on the list.

## After

When it merges, record the date in `decisions.md`.
