-- Secret Discovery scans table
-- Stores scan metadata and results (encrypted)

CREATE TABLE IF NOT EXISTS discovery_scans (
    scan_id VARCHAR(36) PRIMARY KEY,
    org_id UUID NOT NULL REFERENCES organizations(org_id),
    agent_id VARCHAR(255) NOT NULL,
    started_by VARCHAR(255) NOT NULL,
    scope JSONB NOT NULL DEFAULT '{}',
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    approved_by VARCHAR(255),
    approved_at TIMESTAMPTZ,
    result_encrypted BYTEA,          -- E2EE encrypted payload (agent→relay)
    result_metadata JSONB,            -- Plaintext metadata (counts, host, duration)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_discovery_scans_org ON discovery_scans(org_id);
CREATE INDEX IF NOT EXISTS idx_discovery_scans_agent ON discovery_scans(agent_id);
CREATE INDEX IF NOT EXISTS idx_discovery_scans_status ON discovery_scans(status);
CREATE INDEX IF NOT EXISTS idx_discovery_scans_created ON discovery_scans(created_at DESC);
