-- Semantic Infrastructure Map
-- Knowledge graph of infrastructure entities and relationships

-- Nodes (hosts, services, databases, queues, endpoints, secrets, monitors, environments)
CREATE TABLE IF NOT EXISTS infra_map_nodes (
    id VARCHAR(255) PRIMARY KEY,
    org_id UUID NOT NULL REFERENCES organizations(org_id),
    node_type VARCHAR(50) NOT NULL,  -- host, service, database, queue, endpoint, secret_ref, monitor, environment
    data JSONB NOT NULL DEFAULT '{}',  -- Full MapNode serialized
    name VARCHAR(255) NOT NULL,
    labels JSONB DEFAULT '{}',
    environment VARCHAR(50),  -- prod, stage, dev
    criticality VARCHAR(20),  -- low, medium, high, critical
    owner VARCHAR(255),
    -- Source tracking
    discovered_by VARCHAR(255),  -- agent_id that discovered this node
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    snapshot_version BIGINT DEFAULT 1
);

-- Edges (relationships between nodes)
CREATE TABLE IF NOT EXISTS infra_map_edges (
    id VARCHAR(255) PRIMARY KEY,
    org_id UUID NOT NULL REFERENCES organizations(org_id),
    from_id VARCHAR(255) NOT NULL,
    to_id VARCHAR(255) NOT NULL,
    rel_type VARCHAR(50) NOT NULL,  -- HOSTS_SERVICE, SERVICE_USES_DB, etc.
    metadata JSONB DEFAULT '{}',
    discovered_by VARCHAR(255),
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_infra_nodes_org ON infra_map_nodes(org_id);
CREATE INDEX IF NOT EXISTS idx_infra_nodes_type ON infra_map_nodes(node_type);
CREATE INDEX IF NOT EXISTS idx_infra_nodes_name ON infra_map_nodes(name);
CREATE INDEX IF NOT EXISTS idx_infra_nodes_env ON infra_map_nodes(environment);
CREATE INDEX IF NOT EXISTS idx_infra_nodes_search ON infra_map_nodes USING gin(name gin_trgm_ops, data);
CREATE INDEX IF NOT EXISTS idx_infra_edges_org ON infra_map_edges(org_id);
CREATE INDEX IF NOT EXISTS idx_infra_edges_from ON infra_map_edges(from_id);
CREATE INDEX IF NOT EXISTS idx_infra_edges_to ON infra_map_edges(to_id);
CREATE INDEX IF NOT EXISTS idx_infra_edges_rel ON infra_map_edges(rel_type);

-- Snapshots (versioned graph state)
CREATE TABLE IF NOT EXISTS infra_map_snapshots (
    id SERIAL PRIMARY KEY,
    org_id UUID NOT NULL REFERENCES organizations(org_id),
    agent_id VARCHAR(255) NOT NULL,
    snapshot JSONB NOT NULL,
    version BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_infra_snapshots_org ON infra_map_snapshots(org_id);
