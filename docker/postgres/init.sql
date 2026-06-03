-- Create database if not exists (Docker entrypoint already creates it via POSTGRES_DB)
-- This script runs on first initialization

-- Enable extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";

-- Note: Apache AGE extension needs to be installed separately
-- For dev, we'll enable it in the migration if the extension is available
-- CREATE EXTENSION IF NOT EXISTS "age";
-- LOAD 'age';
