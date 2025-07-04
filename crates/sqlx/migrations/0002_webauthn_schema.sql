-- WebAuthn schema additions

-- WebAuthn credentials table
CREATE TABLE IF NOT EXISTS webauthn_credentials (
    credential_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    public_key TEXT NOT NULL,
    aaguid TEXT,
    counter INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,  -- ISO8601 format
    last_used_at TEXT,         -- ISO8601 format
    device_name TEXT,          -- Optional device nickname
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_webauthn_credentials_user_id ON webauthn_credentials(user_id);
CREATE INDEX IF NOT EXISTS idx_users_name ON users(name);