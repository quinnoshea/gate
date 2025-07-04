-- Add role field to users table

-- Add role column with default value
ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'user';

-- Update existing users (if any) to have the first user be admin
-- This is a portable way to update the first user by creation date
UPDATE users 
SET role = 'admin' 
WHERE id = (
    SELECT id 
    FROM users 
    ORDER BY created_at ASC 
    LIMIT 1
);

-- Create index for role lookups
CREATE INDEX IF NOT EXISTS idx_users_role ON users(role);