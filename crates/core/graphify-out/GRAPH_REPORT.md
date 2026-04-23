# Graph Report - /Users/braincoder/Projects/flowlink/crates/core  (2026-04-22)

## Corpus Check
- Corpus is ~9,061 words - fits in a single context window. You may not need a graph.

## Summary
- 244 nodes · 325 edges · 23 communities detected
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 12 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Community 0|Community 0]]
- [[_COMMUNITY_Community 1|Community 1]]
- [[_COMMUNITY_Community 2|Community 2]]
- [[_COMMUNITY_Community 3|Community 3]]
- [[_COMMUNITY_Community 4|Community 4]]
- [[_COMMUNITY_Community 5|Community 5]]
- [[_COMMUNITY_Community 6|Community 6]]
- [[_COMMUNITY_Community 7|Community 7]]
- [[_COMMUNITY_Community 8|Community 8]]
- [[_COMMUNITY_Community 9|Community 9]]
- [[_COMMUNITY_Community 10|Community 10]]
- [[_COMMUNITY_Community 11|Community 11]]
- [[_COMMUNITY_Community 12|Community 12]]
- [[_COMMUNITY_Community 13|Community 13]]
- [[_COMMUNITY_Community 14|Community 14]]
- [[_COMMUNITY_Community 15|Community 15]]
- [[_COMMUNITY_Community 16|Community 16]]
- [[_COMMUNITY_Community 17|Community 17]]
- [[_COMMUNITY_Community 18|Community 18]]
- [[_COMMUNITY_Community 19|Community 19]]
- [[_COMMUNITY_Community 20|Community 20]]
- [[_COMMUNITY_Community 21|Community 21]]
- [[_COMMUNITY_Community 22|Community 22]]

## God Nodes (most connected - your core abstractions)
1. `AgentConfig` - 7 edges
2. `all_codes()` - 5 edges
3. `Message` - 5 edges
4. `RelayConfig` - 4 edges
5. `sample_agent_config()` - 4 edges
6. `test_agent_config_save_load_roundtrip()` - 4 edges
7. `test_message_roundtrip_through_json()` - 4 edges
8. `AuditEvent` - 4 edges
9. `AuditEventType` - 4 edges
10. `VaultClient` - 4 edges

## Surprising Connections (you probably didn't know these)
- `test_agent_config_save_load_roundtrip()` --calls--> `sample_agent_config()`  [EXTRACTED]
  src/config.rs → src/config.rs  _Bridges community 7 → community 8_

## Communities

### Community 0 - "Community 0"
Cohesion: 0.0
Nodes (27): ApprovalRequestPayload, ApprovalResponsePayload, BackupProgressPayload, BackupRequestPayload, BackupResponsePayload, BackupRestorePayload, ConfigUpdatePayload, ConnectedPayload (+19 more)

### Community 1 - "Community 1"
Cohesion: 0.0
Nodes (17): AlertThreshold, AuditEvent, AuditEventType, CanaryToken, ForensicSummary, SessionChunk, test_audit_event_new_generates_uuid(), test_audit_event_serialization() (+9 more)

### Community 2 - "Community 2"
Cohesion: 0.0
Nodes (5): LlmBackend, OAuthConfig, OAuthProviderConfig, PlanConfig, TlsConfig

### Community 3 - "Community 3"
Cohesion: 0.0
Nodes (11): Permission, RbacToken, RbacUser, Role, test_admin_has_all_permissions(), test_agent_permissions(), test_multi_role_union(), test_operator_permissions() (+3 more)

### Community 4 - "Community 4"
Cohesion: 0.0
Nodes (15): Message, test_exec_done_payload_roundtrip(), test_message_deeply_nested_payload(), test_message_extra_fields_ignored(), test_message_new(), test_message_optional_fields_skip(), test_message_roundtrip_through_json(), test_message_serialize_deserialize() (+7 more)

### Community 5 - "Community 5"
Cohesion: 0.0
Nodes (5): all_codes(), test_all_codes_are_non_empty(), test_code_format_uppercase_snake(), test_no_duplicate_codes(), test_no_empty_codes()

### Community 6 - "Community 6"
Cohesion: 0.0
Nodes (7): RelayConfig, AppRoleAuth, AppRoleLoginResponse, KvData, KvResponse, test_vault_client_from_env(), VaultClient

### Community 7 - "Community 7"
Cohesion: 0.0
Nodes (11): default_registry_path(), RegistryConfig, sample_agent_config(), test_agent_config_serialize_deserialize_roundtrip(), test_approval_defaults(), test_backup_defaults(), test_registry_defaults(), test_relay_config_serialize_deserialize() (+3 more)

### Community 8 - "Community 8"
Cohesion: 0.0
Nodes (2): AgentConfig, test_agent_config_save_load_roundtrip()

### Community 9 - "Community 9"
Cohesion: 0.0
Nodes (5): BackupConfig, default_backup_dir(), default_max_snapshots(), default_max_total_size(), default_retention_days()

### Community 10 - "Community 10"
Cohesion: 0.0
Nodes (4): ApprovalConfig, default_approval_mode(), default_hard_ask_timeout(), default_max_retries()

### Community 11 - "Community 11"
Cohesion: 0.0
Nodes (4): default_smtp_from(), default_smtp_host(), default_smtp_port(), SmtpConfig

### Community 12 - "Community 12"
Cohesion: 0.0
Nodes (3): default_max_exec_timeout(), default_max_file_size(), SandboxConfig

### Community 13 - "Community 13"
Cohesion: 0.0
Nodes (3): default_audit_log(), default_shield_timeout(), ShieldConfig

### Community 14 - "Community 14"
Cohesion: 0.0
Nodes (2): default_llm_timeout(), LlmConfig

### Community 15 - "Community 15"
Cohesion: 0.0
Nodes (2): DatabaseConfig, default_db_pool_size()

### Community 16 - "Community 16"
Cohesion: 0.0
Nodes (0): 

### Community 17 - "Community 17"
Cohesion: 0.0
Nodes (1): GithubConfig

### Community 18 - "Community 18"
Cohesion: 0.0
Nodes (1): WssTlsConfig

### Community 19 - "Community 19"
Cohesion: 0.0
Nodes (1): BillingConfig

### Community 20 - "Community 20"
Cohesion: 0.0
Nodes (1): YandexConfig

### Community 21 - "Community 21"
Cohesion: 0.0
Nodes (1): VkConfig

### Community 22 - "Community 22"
Cohesion: 0.0
Nodes (2): request_id(), test_request_id_generates_uuid()

## Knowledge Gaps
- **41 isolated node(s):** `RbacToken`, `OAuthConfig`, `TlsConfig`, `OAuthProviderConfig`, `LlmBackend` (+36 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **Thin community `Community 16`** (2 nodes): `main()`, `bench_core.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 17`** (2 nodes): `GithubConfig`, `.default()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 18`** (2 nodes): `WssTlsConfig`, `.is_enabled()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 19`** (2 nodes): `BillingConfig`, `.default()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 20`** (2 nodes): `YandexConfig`, `.default()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 21`** (2 nodes): `VkConfig`, `.default()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 22`** (2 nodes): `request_id()`, `test_request_id_generates_uuid()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `test_no_duplicate_codes()` connect `Community 5` to `Community 1`?**
  _High betweenness centrality (0.108) - this node is a cross-community bridge._
- **Why does `RelayConfig` connect `Community 6` to `Community 8`, `Community 2`?**
  _High betweenness centrality (0.069) - this node is a cross-community bridge._
- **Why does `AuthConfig` connect `Community 1` to `Community 2`?**
  _High betweenness centrality (0.052) - this node is a cross-community bridge._
- **What connects `RbacToken`, `OAuthConfig`, `TlsConfig` to the rest of the system?**
  _41 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 0` be split into smaller, more focused modules?**
  _Cohesion score 0.04 - nodes in this community are weakly interconnected._
- **Should `Community 1` be split into smaller, more focused modules?**
  _Cohesion score 0.07 - nodes in this community are weakly interconnected._
- **Should `Community 2` be split into smaller, more focused modules?**
  _Cohesion score 0.07 - nodes in this community are weakly interconnected._