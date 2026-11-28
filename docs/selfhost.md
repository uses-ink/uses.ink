# Self-hosting uses.ink

`ink serve` is the whole product in one process: the edge that serves sites, the control plane and
its API, the dashboard, the watch that builds git sites, and the worker that runs the builds
(decision 0005). This page installs it, runs it, backs it up, upgrades it and puts it behind a
reverse proxy. Every flag below is read from `crates/surface/cli/src/grammar/args.rs` and the files
beside it, and `ink serve --help` prints the same list.

Pre-release: no release is published yet, so the first two ways to install below work once the owner
tags one. Until then, build from source.

## Install

| Way | Command | Gets you |
|---|---|---|
| Installer (Linux, macOS, Windows) | `curl --proto '=https' --tlsv1.2 -LsSf https://github.com/uses-ink/uses.ink/releases/latest/download/uses-ink-cli-installer.sh \| sh` | `ink` in `$CARGO_HOME/bin`, from the archive of the release, checked against its `.sha256` |
| Image | `docker pull ghcr.io/uses-ink/uses-ink:latest` | `ink serve` in a container, below |
| Source | `cargo xtask component`, then `cargo install --path crates/surface/cli --locked` | `ink` built here; it embeds its components, so the first command is not optional |

The archives hold one binary, `ink`, and the three licence files. Check one by hand with
`sha256sum -c uses-ink-cli-<target>.tar.xz.sha256`.

## First run, on one machine

```
$ mkdir site && cd site
$ printf '# Hello\n' > index.md
$ ink build
$ ink serve
```

`ink build` writes `.ink/out`, and `ink serve` serves it on `127.0.0.1:8080`, through the same edge
code production runs. That serves one folder and nothing else: the start lists `api` as `off`. The pages
`ink build` writes name `http://127.0.0.1:4000` as their canonical address, the preview port; a
site published with `ink push` is built again on the server with its own address.

To run the product, give it a data folder, and keep that folder outside the site: a data folder
inside the site would be part of what `ink push` sends, so `ink push` refuses with
`push.folder.holds-data` when it finds `ink.db`, `secret.key` or `operator-keys` in the folder.
The folder `.ink/out` is still read, which here is the one `ink build` just wrote. Stop the first
server with Ctrl-C, then, still in `site`:

```
$ ink serve --data ../state
```

The start prints every setting it runs with, since nothing is silent (decision 0005), in this order
(decision 0210):

```
ink 0.1.0  serve  listening on 127.0.0.1:8080

  site       http://localhost:8080
  dashboard  http://api.localhost:8080
  api        http://api.localhost:8080/v1/sites

╭─ admin link  works for 1d, shown once and kept nowhere
│
│ http://api.localhost:8080/login/Zx3kQ9vT2mLp7RwB4nHd6YcAeF8gJs1U
╰─ opening it signs you in as the owner

Addresses
  listen     127.0.0.1:8080           default
  origin     http://localhost:8080    derived (--origin)
  ...
press ctrl-c to stop
```

Read these in it:

- The `admin link` block: the first-run link, shown once, kept nowhere, valid for 24 hours
  (`--admin-ttl`). Opening it signs you in as the owner. A later start makes no second link and says
  `not made`, so on a data folder you have not used yet, a lost link means removing the folder and
  starting again.
- `dashboard`: `http://api.localhost:8080`, the API host. Register a passkey there. A passkey is
  made for one origin, so move to a real name before you make one you want to keep.
- The sections below it (Addresses, Sign-in, Data, Limits, Background work, Security, Mail) list every
  setting with its value and where it came from: `default`, or the flag that changed it. A setting
  that is a warning, such as a cookie that is not `Secure` on plain http, says so with its fix.
- `Data` section: `database` is the file, and `migrations` lists the migrations this start applied
  to it, or says none was due. After an upgrade, that is where to read what changed.

`ink serve --print-config --data ./state` prints the same report from the flags alone and exits. It
binds nothing and makes nothing, so it does not use up the first-run link of a new folder. With
`--json` the start is one document (`uses.serve/v2`) with the addresses, the link and every setting
as typed fields. After the start, the server logs one line for each thing it does: builds, the
watch, mail and failures always, and each request with `-v`. `--log-format json` writes them as one
JSON object a line, `--log-level` and `-q` say how much, and `--log-time local` writes times in the
machine's zone.

Open the admin link in a browser. It shows a form with one field, the email address of the owner
account, and a button; the button signs you in as the owner. From a script, post the form from the
dashboard's own origin, which the server checks (`Origin` must be the dashboard's address):

```
$ curl -c jar -H 'Origin: http://api.localhost:8080' --data-urlencode address=me@example.org \
    http://api.localhost:8080/login/<token>
```

The answer is `303` with a session cookie, and `curl -b jar http://api.localhost:8080/account`
shows the account.

Then, from another terminal in the folder that holds `site` and `state`, enrol the command line as
the operator of this server and publish. `ink login` keeps a key but not the API address, so give
the address to the commands that follow, here as the environment variable they all read:

```
$ ink login --data ./state --api http://api.localhost:8080
$ export INK_API=http://api.localhost:8080
$ ink -C site init --handle garden
$ ink -C site push
```

The site is served at `http://garden.localhost:8080`. `*.localhost` names resolve to the loopback
address in current browsers and, on the hosts this was run on, in `curl` and in `ink` itself;
elsewhere, add the names to `/etc/hosts`, or point `curl` at them with
`--resolve garden.localhost:8080:127.0.0.1`.

After a start, the server refuses signed requests for about 30 seconds: it keeps the nonces it has
seen in memory, so it cannot tell a replay of a request signed before the restart. An `ink` call in
that time fails with `auth.server.restarting`, the seconds to wait, and the fix: run it again. Sites
are served at once.

From the same terminal, the operator's other commands work against this server:

```
$ ink org create "Garden"            # an organization you own
$ ink -C site domain add www.example.org   # prints the TXT record to publish
$ ink -C site preview list           # previews of git branches; none for an upload site
$ ink abuse list                     # the operator's key; reports of strangers
```

## Behind a reverse proxy, on a real name

`ink serve` speaks plain http. A proxy ends TLS, and a non-local `--origin` must be `https://`:
a plain `http://` origin on a name that is not local is refused at start with the reason.

```
mkdir -p /var/lib/ink/www
ink serve /var/lib/ink/www --addr 127.0.0.1:8080 --data /var/lib/ink/state \
  --origin https://example.org --api-host api.example.org --forwarded-hops 1
```

The first argument is the folder of built deploys that `ink serve` also serves, `.ink/out` of the
working directory when it is left out. It must exist or the start fails with
`serve.folder.unservable`, so give it an empty one: the sites of the control plane come from `--data`.

- `--origin` is where sites are served: `<handle>.example.org`. It is also the scheme and port the
  API reports, so give the address a browser uses.
- `--api-host` is the one host name whose requests are API and dashboard calls (default
  `api.localhost`). Passkeys are made for it (`--rp-id` changes that, and a passkey made under one
  id never works under another).
- Requests of `ink` are signed for the API host on the origin's port, none for `https`, whatever
  port `--addr` listens on. `--audience <host[:port]>` says another name, for a proxy that does not
  pass the `Host` header the client wrote; the start prints the audience it runs with.
- `--forwarded-hops N` says how many proxies of yours sit in front. With it the client address is
  read from `X-Forwarded-For`, `N` entries from the end, and a request without the header is
  refused; the sign-in and abuse limits count per client address, so without it every request looks
  like it came from the proxy.

The proxy needs, for the names `example.org`, `api.example.org` and `*.example.org`: DNS records to
the proxy, a certificate that covers a wildcard (which needs a DNS challenge of your DNS provider),
the `Host` header passed through unchanged, and `X-Forwarded-For` set. A custom domain of a site
(`ink domain add`) is a name of its own, so its certificate must be issued when the name is proved.
A Caddy file of that shape, not run here:

```
example.org, api.example.org, *.example.org {
	tls {
		dns <your provider>
	}
	reverse_proxy 127.0.0.1:8080
}
```

## Container image

The `Dockerfile` at the root builds the same `dist` profile as the release archives, on musl, into a
busybox image that runs as user `65532`, with `/data` as the only writable path. The image's
default command serves the folder `/data/.ink/out`, which the image makes empty, and nothing else, so
a compose file names the flags, as below.

```yaml
services:
  ink:
    image: ghcr.io/uses-ink/uses-ink:latest   # or `build: .` in a checkout
    command:
      - serve
      - --addr=0.0.0.0:8080
      - --data=/data/state
      - --origin=https://example.org
      - --api-host=api.example.org
      - --forwarded-hops=1
      - --mail=smtp
      - --mail-from=ink@example.org
      - --smtp-host=mail.example.org
      - --smtp-user=ink@example.org
    environment:
      INK_SMTP_PASSWORD_FILE: /run/secrets/smtp
    secrets: [smtp]
    read_only: true
    tmpfs: [/tmp]
    cap_drop: [ALL]
    security_opt: ["no-new-privileges:true"]
    volumes:
      - ink-data:/data
    ports:
      - "127.0.0.1:8080:8080"
    restart: unless-stopped

volumes:
  ink-data:

secrets:
  smtp:
    file: ./smtp-password
```

What is checked and what is not. `cargo xtask release image` (run on a build host through the farm)
builds the image, reads its user and healthcheck, starts it read-only with every capability dropped,
a fresh volume at `/data` and `serve --addr 0.0.0.0:8080 --data /data/state`, and requires
`/healthz` to answer. The compose file above was run as the equivalent `podman run` (flags, the
`INK_SMTP_PASSWORD_FILE` secret mounted at `/run/secrets/smtp`, the read-only root, `--cap-drop ALL`,
`no-new-privileges`, a named volume, `127.0.0.1:8080` published) against an SMTP host that does not
exist: the start printed its settings, `/healthz` answered `200` and the container's own healthcheck
passed. No mail was sent, so the SMTP login itself is not checked.

Get the first-run link and enrol the command line, in a container that has no shell and whose data
folder you do not read from outside:

```
$ docker logs ink | grep 'admin link'      # open it, give an address, sign in
$ ink login --api https://api.example.org  # prints a code and a fingerprint, and waits
```

`ink login` without `--data` asks for approval: it prints a code and a fingerprint, and waits. The
dashboard has no page that approves one yet. The address `ink login` prints, `/v1/approve/<code>`,
answers a `GET` from a signed-in browser with JSON about the request, and the approval is a `POST` to
it with the fingerprint, from the dashboard's own origin and the session of the first-run link:

```
$ curl -b jar -H 'Origin: https://api.example.org' -H 'Content-Type: application/json' \
    -d '{"fingerprint":"ABCD-EFGH-IJ"}' https://api.example.org/v1/approve/<code>
```

Compare the fingerprint with the one `ink login` printed first. The key is a device key of the owner
account (it lives 180 days). `ink login --data <folder>` is for a data folder you can write on the
machine you run `ink` on. After approval, `ink init` and `ink push` into the container worked, and
the site was served by its host name.

The gap, as it is now. `ink login` tells the person to open the address it prints, and that address
is not a page: the dashboard has no route that shows the key's label and fingerprint and asks for
the comparison, and a `GET` of `/v1/approve/<code>` answers JSON. What a page has to do is already
on the API: `GET /v1/approve/<code>` for the label and fingerprint (session cookie required, no
signature), and `POST` of `{"fingerprint": ...}`, with `Origin` equal to the dashboard's address, to
approve. A body that is not that JSON is refused with `400` and the code `auth.approval.malformed`,
and the wrong fingerprint with `409`. Two more things to fix with the page: the message `ink login`
prints says to open the address and approve there, which is false until the page exists, and a code
that is not found counts toward the guess limit that closes enrolling for a minute, so a page must
not probe codes.

- The image's healthcheck asks `/healthz` on `127.0.0.1`, which answers `200` only for a host no site
  has (decision 0051), so it works with sites and an API host of any name.
- Without `--mail smtp`, sign-in mail is printed to the container's standard output, links
  included: `docker logs` is where the first-run link is on a first run.
- A bind mount at `/data` must be owned by `65532:65532`. A named volume takes the image's owner.
- Nothing needs a shell in the image; to run `ink` from it, name the command:
  `docker run --rm uses-ink --version`.

## What the data folder holds

| Path under `--data` | What |
|---|---|
| `ink.db`, `ink.db-wal`, `ink.db-shm` | The database (SQLite, WAL mode): accounts, keys, sites, deploys, the build queue |
| `secret.key` | The key that seals stored credentials (decision 0031). It is created on the first start and lives beside the database |
| `blobs/` | Every file of every deploy, named by its hash |
| `operator-keys` | The public keys of the operators, enrolled by `ink login --data`. No secret is in it |
| `git/` | Clones and the last heads seen of git sites; `--git-cache` moves it |
| `components/` | The components the builder runs |

## Backup and restore

A backup is the whole data folder, taken while `ink serve` is stopped. The database runs in WAL
mode, so a copy of `ink.db` alone, taken while the server runs, can miss what is still in
`ink.db-wal`. `secret.key` belongs to the same backup as the database: sealed credentials cannot be
read without it, and a key stored beside the backup leaks nothing a stolen database does not, since
the key is the only thing that opens them. Keep the backup where only the operator reads.

```
$ docker compose stop ink
$ docker run --rm -v ink-data:/data -v "$PWD":/backup busybox:1.37 tar -C /data -czf /backup/ink-data.tar.gz .
$ docker compose start ink
```

To restore, stop the server, empty the volume, unpack the archive into it as `65532:65532` and start.
On a bare install the same two steps are `cp -a` or `tar` on the folder `--data` names. After a
restore, `ink status` on a site is the check, once the server takes signed requests again (about 30
seconds after the start). A start from a copy makes no second admin link (`admin link  not made`),
and the sites, the accounts and their sessions are there.

## Upgrade

Starting a newer `ink` applies the database migrations before it serves, and says so in the
`migrations` row of the start's Data section: the migrations applied now, or that none was due.
There is no downgrade: the way back is the backup taken before. So:

1. Take a backup, as above.
2. Replace the binary, or `docker compose pull && docker compose up -d`.
3. Read the start's listing for a setting whose default changed, and `ink status` for the sites.

## What was run, and what was not

`cargo xtask smoke` runs the first-run, backup and upgrade sections above with the real `ink`, on
one machine, in `target/smoke`, with a free port and a home folder of its own for the keys; on a
build host it is `farm cargo xtask smoke`. It builds the components and `ink` first, so a cold run is
a build and about a minute of stages. `cargo xtask smoke <path to an older ink>` adds the upgrade stage:
that `ink` makes a data folder with a site, and this one starts it. The container is
`cargo xtask release image`'s.

Run on 2026-11-28 on a Linux x86_64 host (farm-a, podman 5.8.9). The rows marked smoke are
`cargo xtask smoke` on the tree of the commit that holds this page, with an `ink` of the commit
before branch previews as the older one. The container rows ran on the same tree, by hand.

| Step | Result |
|---|---|
| `cargo xtask component`, `ink` built (debug and release) | passed |
| `ink build`, `ink serve`, `GET /` (smoke) | passed; the page names `127.0.0.1:4000` as canonical, as said above |
| `ink serve --data`, the first-run link, sign-in, `/account` (smoke) | passed, with the address field and the `Origin` header said above |
| `ink login --data`, `init`, `push`, `garden.localhost` (smoke) | passed once `INK_API` is set; before this page said so, `init` stopped with `deploy.setting.missing` |
| `org create`, `org list`, `domain add` (the TXT record), `preview list`, `abuse list` (smoke) | passed |
| Backup (`cp -a` of a stopped data folder) and a start from the copy (smoke) | passed: site, account, session, no second admin link |
| Upgrade: a data folder of an older build started with this `ink` (smoke, older `ink` named) | passed: the `migrations` row names them, the old site is served |
| Image built, read-only, no capabilities, `/healthz`, healthcheck (`release image`) | passed; the build failed on `main` for a while on a missing `brand/` in the build context, fixed in `.dockerignore` |
| The compose flags as `podman run`, `/healthz`, device login, approval, `push` into the container | passed by hand, with the API published on a port other than the one listened on (`-p 18082:8080`, `--origin http://localhost:18082`); the approval was a `POST` with `curl` (no dashboard page does it yet) |

Not run: the installer script and the archives (no release is published), the image from `ghcr.io`,
`docker` itself (podman was used), the compose file through `docker compose`, a real reverse proxy
with TLS and the Caddy file, a wildcard certificate, sending mail through SMTP, a passkey (it needs a
browser), `ink domain prove` against real DNS, git sites and forge hooks, the container backup and
restore with `tar`, and the platforms other than Linux x86_64.

## Limits and flags

Each flag has a default that the start prints. Flags with an environment variable are settable
there; the rest are flags only.

### Server

| Flag | Default | What |
|---|---|---|
| `--addr` | `127.0.0.1:8080` | Where to listen |
| `--data <folder>` | none | Turns on the API, dashboard, watch and worker |
| `--host-mode` | `auto` | How a host picks a deploy: `single`, `folders` or `auto` |
| `--origin` | `http://localhost:<port>` | Where sites are served, and the passkey origin |
| `--api-host` | `api.localhost` | The host of the API and dashboard |
| `--forwarded-hops` (`INK_FORWARDED_HOPS`) | off | Proxies in front |
| `--signup` (`INK_SIGNUP`) | `first` | Who may make an account: `first`, `open` or `closed` |

### Sizes and rates

| Flag | Default | What |
|---|---|---|
| `--api-body` | 65536 B | Largest JSON request body |
| `--upload-bytes` | 268435456 B | Largest uploaded archive |
| `--upload-files` | 10000 | Most files in an uploaded archive |
| `--upload-file` | 67108864 B | Largest file in an archive |
| `--upload-tree` | 8388608 B | Largest tree `ink push` sends |
| `--upload-part` | 67108864 B | Most bytes of files in one push request, held in memory |
| `--export-bytes` | 268435456 B | Largest exported archive |
| `--poll-interval` | 600 s | Between polls of a git source; at least 10 |
| `--poll-max` | 3600 s | A source that does not change is backed off to this |
| `--rate-address` (`INK_RATE_ADDRESS`) | `5/1h` | Sign-in links per address |
| `--rate-source` (`INK_RATE_SOURCE`) | `20/1h` | Sign-in links per client address |
| `--abuse-limit` (`INK_ABUSE_LIMIT`) | `5/1h` | Abuse reports per client address |
| `--enrol-limit` (`INK_ENROL_LIMIT`) | `5/1h` | Key enrolments asked per client address |

### Accounts and sites

| Flag | Default | What |
|---|---|---|
| `--limit-stored` (`INK_LIMIT_STORED`) | 1073741824 B | Bytes an account may store; a deploy over it is refused whole |
| `--limit-builds` (`INK_LIMIT_BUILDS`) | 60 | Builds an account may start in 24 hours |
| `--limit-sites` (`INK_LIMIT_SITES`) | 10 | Sites an account may have |
| `--limit-deploys` (`INK_LIMIT_DEPLOYS`) | 20 | Deploys of a site that are kept |
| `--limit-build-time` (`INK_LIMIT_BUILD_TIME`) | 600 s | One build; a longer one is given to another worker |
| `--limit-domains` (`INK_LIMIT_DOMAINS`) | 10 | Custom domains of a site |
| `--limit-reports` (`INK_LIMIT_REPORTS`) | 10000 | Abuse reports open at once (decision 0152) |
| `--preview-days` (`INK_PREVIEW_DAYS`) | 7 | Days a branch preview is served |
| `--preview-branches` (`INK_PREVIEW_BRANCHES`) | 10 | Branches of a public git site built as previews; `0` builds none |
| `--handle-hold` (`INK_HANDLE_HOLD`) | 365 days | A handle a site left is held this long |
| `--blob-grace` (`INK_BLOB_GRACE`) | 900 s | A blob no deploy names is deleted after this |

### Git, DNS and the loops

| Flag | Default | What |
|---|---|---|
| `--git-scheme` | `https` | Kinds of git address a site may be added with; `file` serves a self-host that builds from its own disk |
| `--git-time`, `--git-connect`, `--git-request` | 120 s, 10 s, 120 s | Time for a fetch, a connection, one request |
| `--git-ca <PEM>` | none | An extra authority to trust, for a private forge |
| `--resolver` (`INK_RESOLVER`) | `system` | How custom-domain proof records are read; `offline` asks none |
| `--resolver-server`, `--resolver-confirm` | system | Nameservers to ask, and a second set that must agree |
| `--forge-hooks` (`INK_FORGE_HOOKS`) | off | Takes `POST /v1/hooks/{github,forgejo,gitlab}` and lets `ink repo` claim a site's repository (decisions 0160 to 0162) |
| `--forge-instances <FILE>`, `--forge-gitlab <HOST>` | bundled list, `gitlab.com` | The Forgejo and GitLab hosts the server may call; any other host is refused before a request is sent |
| `--forge-route HOST=ADDR` | none | Send calls for a forge name to an address, for a mirror; the certificate is still checked for the name |
| `--forge-timeout`, `--forge-reply` | 10 s, 1 MiB | Time for one call to a forge, and the most bytes read of a reply |
| `--forge-body`, `--forge-window`, `--forge-remembered` | 5 MiB, 600 s, 10000 | The largest webhook body, how long a delivery id is remembered, and how many are kept for one repository |
| `--restart-limit` | 5 | Crashes in a row before the watch or worker is left stopped |
| `--restart-delay`, `--restart-healthy` | 2 s, 60 s | Wait before a restart, and the run that forgets earlier crashes |
| `--worker-lease`, `--worker-idle` | 900 s, 2 s | How long a worker holds a request, and rests when the queue is empty |

### Mail and sessions

| Flag | Default | What |
|---|---|---|
| `--mail` (`INK_MAIL`) | `stdout` | `stdout` prints links; `smtp` sends them |
| `--mail-from`, `--smtp-host` | none | Required with `--mail smtp` |
| `--smtp-port` | 587, or 465 with `--smtp-security implicit` | |
| `--smtp-security` | `starttls` | `starttls`, `implicit` or `plain` |
| `--smtp-user`, `--smtp-password-file` | none | The password is `INK_SMTP_PASSWORD` or a file, never a flag |
| `--login-ttl`, `--admin-ttl` | 15m, 24h | How long a sign-in link and the first-run link work |
| `--session-expiry`, `--session-idle`, `--session-limit` | `sliding`, 7d, 30d | How sessions end |
| `--step-up-window` | 5m | How recent a passkey sign-in must be for a sensitive change |

A flag that is wrong stops the start and names the flag. `ink serve --help` lists the rest (keys,
cookies, passkey origins).
