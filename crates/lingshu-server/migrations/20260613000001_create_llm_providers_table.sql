-- LLM providers: per-user saved provider configurations.
-- Built-in providers are defined in code; this table stores user-specific
-- credentials (API keys, custom base URLs) and custom provider entries.
CREATE TABLE llm_providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    provider_id VARCHAR(50) NOT NULL,
    display_name VARCHAR(100) NOT NULL,
    api_key_encrypted BYTEA,
    api_base_url TEXT,
    models JSONB NOT NULL DEFAULT '[]',
    is_active BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_llm_providers_user ON llm_providers(user_id) WHERE deleted_at IS NULL;

-- Each user can have at most one entry per provider_id (e.g. one "openai",
-- one "deepseek", and multiple "custom" entries distinguished by display_name).
-- For built-in providers the unique constraint is on (user_id, provider_id).
-- For custom providers we allow multiple rows with provider_id = 'custom',
-- distinguished by display_name, so no unique constraint on provider_id alone.
