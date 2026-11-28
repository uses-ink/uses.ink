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
code production runs. That serves one folder and nothing else: the start says `api: off`.

To run the product, give it a data folder. The folder `.ink/out` is still read, which here is the
one `ink build` just wrote:

```
$ ink serve --data ./state
```

The start prints every setting it runs with and the default it chose, since nothing is silent
(decision 0005). Read two lines of it:

- `admin link`: the first-run link, shown once, kept nowhere, valid for 24 hours (`--admin-ttl`).
  Opening it signs you in as the owner. A later start makes no second link, so on a data
  folder you have not used yet, a lost link means removing the folder and starting again.
- `dashboard`: `http://api.localhost:8080`, the API host. Register a passkey there. A passkey is
  made for one origin, so move to a real name before you make one you want to keep.

Then, from another terminal, enrol the command line as the operator of this server and publish:

```
$ ink login --data ./state --api http://api.localhost:8080
$ ink -C site init --handle garden
$ ink -C site push
```

The site is served at `http://garden.localhost:8080`. `*.localhost` names resolve to the loopback
address in current browsers and in `curl`; elsewhere, add the names to `/etc/hosts`.

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
`/healthz` to answer. The compose file above was not run; its flags are the ones that check passes,
plus the mail ones this page takes from the source.

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
On a bare install the same two steps are `tar` on the folder `--data` names. The restore is not
covered by a test here: after one, `ink status` and `ink deploy list` on a site are the check.

## Upgrade

Starting a newer `ink` applies the database migrations before it serves, and says so. There is no
downgrade: the way back is the backup taken before. So:

1. Take a backup, as above.
2. Replace the binary, or `docker compose pull && docker compose up -d`.
3. Read the start's listing for a setting whose default changed, and `ink status` for the sites.

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
