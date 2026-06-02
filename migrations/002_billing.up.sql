-- 002_billing.up.sql — China payments tables
-- cn_orders: order state machine (pending→paid→license_issued→confirmed→refunded)
-- cn_idempotency: callback dedup via INSERT OR IGNORE
-- cn_fapiao: tax invoices for enterprise

CREATE TABLE IF NOT EXISTS cn_orders (
    order_id    TEXT PRIMARY KEY NOT NULL,
    plan        TEXT NOT NULL,
    amount_cny  INTEGER NOT NULL,
    gateway     TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',
    user_email  TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    paid_at     TEXT,
    refunded_at TEXT,
    license_key TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS cn_idempotency (
    idempotency_key TEXT PRIMARY KEY NOT NULL,
    order_id       TEXT NOT NULL,
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (order_id) REFERENCES cn_orders(order_id)
) STRICT;

CREATE TABLE IF NOT EXISTS cn_fapiao (
    fapiao_id   TEXT PRIMARY KEY NOT NULL,
    order_id    TEXT NOT NULL,
    fapiao_type TEXT NOT NULL DEFAULT 'electronic',
    title       TEXT NOT NULL,
    tax_id      TEXT,
    amount_cny  INTEGER NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',
    issued_at   TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (order_id) REFERENCES cn_orders(order_id)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_cn_orders_status ON cn_orders (status);
CREATE INDEX IF NOT EXISTS idx_cn_orders_user ON cn_orders (user_email);
