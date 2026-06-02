-- 002_billing.down.sql — Rollback billing tables
DROP TABLE IF EXISTS cn_fapiao;
DROP TABLE IF EXISTS cn_idempotency;
DROP TABLE IF EXISTS cn_orders;
