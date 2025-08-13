-- Permissions table for RBAC
CREATE TABLE IF NOT EXISTS permissions (
    subject_id TEXT NOT NULL,
    action TEXT NOT NULL,
    object TEXT NOT NULL,
    granted_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    granted_by TEXT,
    PRIMARY KEY (subject_id, action, object)
);

CREATE INDEX IF NOT EXISTS idx_permissions_subject ON permissions(subject_id);
CREATE INDEX IF NOT EXISTS idx_permissions_object ON permissions(object);