-- 001_license.down.sql — Rollback license tables
DROP TABLE IF EXISTS license_activation_failures;
DROP TABLE IF EXISTS license_daily_usage;
DROP TABLE IF EXISTS license_meta;
