-- Zero-Trust Secret Configuration per organization
-- Stores ONLY public keys and vault config — never private keys

CREATE TABLE IF NOT EXISTS org_secret_configs (
    org_id UUID PRIMARY KEY REFERENCES organizations(org_id),
    -- Organization's X25519 public key (base64, 32 bytes)
    org_public_key TEXT,
    -- Key ID (first 16 hex chars of SHA-256 of public key)
    org_key_id TEXT,
    -- Vault mode: 'embedded' | 'external' | 'none'
    vault_mode TEXT NOT NULL DEFAULT 'none',
    -- Vault configuration JSON (VaultMode serialized)
    vault_config JSONB,
    -- Who set up the key
    key_set_up_by TEXT,
    -- When the key was last rotated
    key_rotated_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_org_secret_configs_key_id ON org_secret_configs(org_key_id);
