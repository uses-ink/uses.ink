-- The control plane's rows (architecture section 11), one set for D1 and SQLite.
-- Times are unix milliseconds. Ids are 32 lower-case hex digits. A deploy's id is the hash of
-- its manifest. Nothing here is derived: the Site, decisions and rendered output are not stored.

CREATE TABLE account (
    id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL
);

CREATE TABLE site (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES account (id),
    handle TEXT NOT NULL UNIQUE,
    -- `removing` is the marker a deletion leaves first, so a crash is diagnosable (decision 0022)
    state TEXT NOT NULL CHECK (state IN ('active', 'removing')),
    created_at INTEGER NOT NULL
);
CREATE INDEX site_account ON site (account_id);

CREATE TABLE source (
    site_id TEXT PRIMARY KEY REFERENCES site (id),
    kind TEXT NOT NULL CHECK (kind IN ('folder', 'git')),
    locator TEXT NOT NULL,
    revision TEXT
);

-- A removed site's handle stays unclaimable
CREATE TABLE tombstone (
    handle TEXT PRIMARY KEY,
    site_id TEXT NOT NULL,
    removed_at INTEGER NOT NULL
);

-- The manifest, which lists every route and blob of the deploy, is the blob whose hash is `id`:
-- the rows say only that the deploy exists and what it cost.
CREATE TABLE deploy (
    site_id TEXT NOT NULL REFERENCES site (id),
    id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    job_id TEXT,
    files INTEGER NOT NULL,
    bytes INTEGER NOT NULL,
    PRIMARY KEY (site_id, id)
);

-- The one pointer a site serves from. `previous_id` is where a rollback goes back to;
-- `paused` is set by a rollback and cleared by `resume`, and while it is set a new deploy is
-- recorded and not published.
CREATE TABLE site_live (
    site_id TEXT PRIMARY KEY REFERENCES site (id),
    deploy_id TEXT NOT NULL,
    previous_id TEXT,
    paused INTEGER NOT NULL DEFAULT 0 CHECK (paused IN (0, 1)),
    set_at INTEGER NOT NULL,
    FOREIGN KEY (site_id, deploy_id) REFERENCES deploy (site_id, id)
);

-- Every move of the pointer, newest last: the history rollback reads.
CREATE TABLE publication (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    site_id TEXT NOT NULL REFERENCES site (id),
    deploy_id TEXT NOT NULL,
    previous_id TEXT,
    cause TEXT NOT NULL CHECK (cause IN ('publish', 'rollback')),
    at INTEGER NOT NULL,
    FOREIGN KEY (site_id, deploy_id) REFERENCES deploy (site_id, id)
);

CREATE TABLE job (
    id TEXT PRIMARY KEY,
    site_id TEXT NOT NULL REFERENCES site (id),
    trigger TEXT NOT NULL CHECK (trigger IN ('manual', 'push', 'poll', 'upload')),
    state TEXT NOT NULL CHECK (state IN ('queued', 'running', 'succeeded', 'refused', 'failed')),
    requested_at INTEGER NOT NULL,
    finished_at INTEGER,
    deploy_id TEXT,
    -- the hash of the build's NDJSON log, a blob of the site's tenant
    log TEXT
);
CREATE INDEX job_site ON job (site_id, requested_at);

-- Credentials are sealed with the row's id as associated data (decision 0031); the sealed bytes
-- are never read back except to open them.
CREATE TABLE credential (
    id TEXT PRIMARY KEY,
    site_id TEXT NOT NULL REFERENCES site (id),
    kind TEXT NOT NULL CHECK (kind IN ('git-token', 'webhook-secret')),
    sealed BLOB NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX credential_site ON credential (site_id);

-- The site's append-only log: one past-tense name and a JSON body per change.
CREATE TABLE event (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    site_id TEXT NOT NULL,
    at INTEGER NOT NULL,
    name TEXT NOT NULL,
    body TEXT NOT NULL
);
CREATE INDEX event_site ON event (site_id, seq);
