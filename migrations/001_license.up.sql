-- 001_license.up.sql — License domain tables
-- license_meta: persistent key-value store
-- license_daily_usage: per-day quota tracking (Asia/Shanghai)
-- license_activation_failures: rate-limit failed activations (max 5/day)

CREATE TABLE IF NOT EXISTS license_meta (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS license_daily_usage (
    date          TEXT PRIMARY KEY NOT NULL,
    request_count INTEGER NOT NULL DEFAULT 0,
    token_count   INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE IF NOT EXISTS license_activation_failures (
    date       TEXT PRIMARY KEY NOT NULL,
    fail_count INTEGER NOT NULL DEFAULT 0
) STRICT;

INSERT OR IGNORE INTO license_meta (key, value) VALUES ('schema_version', '1');
