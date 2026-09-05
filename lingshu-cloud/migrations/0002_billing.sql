PRAGMA foreign_keys = ON;

CREATE TABLE billing_customers (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider = 'stripe'),
    provider_customer_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE subscriptions (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider = 'stripe'),
    provider_customer_id TEXT NOT NULL,
    provider_subscription_id TEXT NOT NULL UNIQUE,
    price_id TEXT,
    plan_code TEXT NOT NULL,
    billing_interval TEXT,
    status TEXT NOT NULL,
    current_period_end TEXT,
    cancel_at_period_end INTEGER NOT NULL DEFAULT 0 CHECK (cancel_at_period_end IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (provider_customer_id) REFERENCES billing_customers (provider_customer_id)
        ON UPDATE CASCADE
);

CREATE INDEX idx_subscriptions_status_period
    ON subscriptions (status, current_period_end);

CREATE TABLE billing_events (
    id TEXT PRIMARY KEY NOT NULL,
    event_type TEXT NOT NULL,
    event_created_at TEXT,
    status TEXT NOT NULL CHECK (status IN ('processing', 'processed', 'failed')),
    error_code TEXT,
    created_at TEXT NOT NULL,
    processed_at TEXT
);

CREATE INDEX idx_billing_events_status_created
    ON billing_events (status, created_at);
