-- Initial schema for Gate (Portable SQL)

-- Users table
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    email TEXT,
    name TEXT,
    created_at TEXT NOT NULL,  -- ISO8601 format
    updated_at TEXT NOT NULL   -- ISO8601 format
);

-- Organizations table
CREATE TABLE IF NOT EXISTS organizations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,  -- ISO8601 format
    settings TEXT              -- JSON as text
);

CREATE TABLE IF NOT EXISTS grants (
    user_id TEXT NOT NULL,    -- which user has the grant
    permission TEXT NOT NULL, -- 'hellas/admin', 'hellas/read',
    created_at TEXT NOT NULL,    -- ISO8601 format
    PRIMARY KEY (user_id, permission)
) WITHOUT ROWID;

-- API Keys table
CREATE TABLE IF NOT EXISTS api_keys (
    key_hash TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    org_id TEXT NOT NULL,
    config TEXT,               -- JSON as text
    created_at TEXT NOT NULL,  -- ISO8601 format
    last_used_at TEXT          -- ISO8601 format
);

-- Providers table
CREATE TABLE IF NOT EXISTS providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    provider_type TEXT NOT NULL,   -- 'openai', 'anthropic', 'google', 'local', 'custom'
    config TEXT,                   -- JSON as text
    enabled INTEGER NOT NULL DEFAULT 1,  -- SQLite uses INTEGER for boolean
    priority INTEGER NOT NULL DEFAULT 0
);

-- Models table
CREATE TABLE IF NOT EXISTS models (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL,
    name TEXT NOT NULL,
    model_type TEXT NOT NULL,      -- 'chat', 'completion', 'embedding', 'image', 'audio'
    capabilities TEXT,             -- JSON as text
    pricing_id TEXT,
    pricing_config TEXT            -- JSON as text
);

-- Usage records table
CREATE TABLE IF NOT EXISTS usage_records (
    id TEXT PRIMARY KEY,
    org_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    api_key_hash TEXT NOT NULL,
    request_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    total_tokens INTEGER NOT NULL,
    cost REAL NOT NULL,           -- REAL is portable (DOUBLE PRECISION is PostgreSQL-specific)
    timestamp TEXT NOT NULL,       -- ISO8601 format
    metadata TEXT                  -- JSON as text
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_usage_records_org_id ON usage_records(org_id);
CREATE INDEX IF NOT EXISTS idx_usage_records_timestamp ON usage_records(timestamp);
CREATE INDEX IF NOT EXISTS idx_api_keys_org_id ON api_keys(org_id);