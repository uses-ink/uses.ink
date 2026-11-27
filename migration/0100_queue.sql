-- The build queue of a self-hosted install (queue/local). At-least-once: a message is hidden
-- while leased (`visible_at` in the future) and delivered again when the lease ends.
CREATE TABLE queue_message (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    body BLOB NOT NULL,
    -- a key makes a message once while it is in the queue; keyless messages are never deduped
    dedupe TEXT UNIQUE,
    -- unix milliseconds
    visible_at INTEGER NOT NULL,
    attempt INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX queue_message_visible ON queue_message (visible_at, id);
