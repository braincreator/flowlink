# Graph Report - /Users/braincoder/Projects/flowlink/crates/relay  (2026-04-22)

## Corpus Check
- 60 files · ~77,622 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1290 nodes · 3297 edges · 31 communities detected
- Extraction: 72% EXTRACTED · 28% INFERRED · 0% AMBIGUOUS · INFERRED: 930 edges (avg confidence: 0.8)
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
- [[_COMMUNITY_Community 23|Community 23]]
- [[_COMMUNITY_Community 24|Community 24]]
- [[_COMMUNITY_Community 25|Community 25]]
- [[_COMMUNITY_Community 26|Community 26]]
- [[_COMMUNITY_Community 27|Community 27]]
- [[_COMMUNITY_Community 28|Community 28]]
- [[_COMMUNITY_Community 29|Community 29]]
- [[_COMMUNITY_Community 30|Community 30]]

## God Nodes (most connected - your core abstractions)
1. `build_router()` - 31 edges
2. `handle_ws()` - 30 edges
3. `handle_command()` - 30 edges
4. `test_handler()` - 26 edges
5. `test_state()` - 25 edges
6. `test_app()` - 22 edges
7. `Registry` - 20 edges
8. `AuthEngine` - 20 edges
9. `encode()` - 18 edges
10. `EmailService` - 18 edges

## Surprising Connections (you probably didn't know these)
- `spawn_relay()` --calls--> `build_router()`  [INFERRED]
  tests/integration_live.rs → src/server.rs
- `spawn_wss_relay()` --calls--> `build_router()`  [INFERRED]
  tests/wss_e2e.rs → src/server.rs
- `handle_fallback()` --calls--> `json_error()`  [INFERRED]
  src/server.rs → src/middleware.rs
- `signup()` --calls--> `bind_default_policy()`  [INFERRED]
  src/control_plane.rs → src/policy_db.rs
- `list_org_audit()` --calls--> `require_org_role()`  [INFERRED]
  src/webhooks_api.rs → src/orgs_api.rs

## Communities

### Community 0 - "Community 0"
Cohesion: 0.0
Nodes (73): check_expiry_bg(), mime_guess(), serve_dashboard(), serve_dashboard_root(), serve_file(), serve_file_with_banner(), Relay, shutdown_signal() (+65 more)

### Community 1 - "Community 1"
Cohesion: 0.0
Nodes (91): AgentHealthOverview, get_latest(), get_timeseries(), gp(), HealthMetric, HealthTimePoint, MetricsQuery, overview() (+83 more)

### Community 2 - "Community 2"
Cohesion: 0.0
Nodes (68): cancel_deletion(), cleanup_expired_deletions(), extract_account_id(), get_pool(), hard_delete(), request_deletion(), fetch_github_user(), fetch_vk_user() (+60 more)

### Community 3 - "Community 3"
Cohesion: 0.0
Nodes (59): check_2fa(), complete_2fa(), CompleteRequest, create_temp_token(), disable_2fa(), DisableRequest, enable_2fa(), EnableRequest (+51 more)

### Community 4 - "Community 4"
Cohesion: 0.0
Nodes (60): cmd_shield(), pattern_matches(), account_info(), account_update_settings(), admin_change_plan(), admin_dashboard_stats(), admin_list_orders(), admin_list_plans() (+52 more)

### Community 5 - "Community 5"
Cohesion: 0.0
Nodes (42): bench_device_pairing(), confirm_pairing(), ConfirmRequest, Device, DeviceManager, DevicesQuery, get_device_trust(), list_devices() (+34 more)

### Community 6 - "Community 6"
Cohesion: 0.0
Nodes (41): extract_api_key(), get_arg(), handle_mcp(), handle_tools_call(), mcp_agents(), mcp_approve(), mcp_config_update(), mcp_deregister() (+33 more)

### Community 7 - "Community 7"
Cohesion: 0.0
Nodes (36): routes(), accept_invite(), AcceptInviteRequest, change_member_role(), ChangeRoleRequest, create_org(), CreateOrgRequest, delete_org() (+28 more)

### Community 8 - "Community 8"
Cohesion: 0.0
Nodes (24): change_email_confirm(), change_email_start(), Claims, is_valid_email(), send_code(), SendCodeRequest, VerifyCodeRequest, EmailService (+16 more)

### Community 9 - "Community 9"
Cohesion: 0.0
Nodes (18): ApiKeyInfo, ApiKeyRepo, ApiKeyRole, ApiKeyWithSecret, generate_key(), hash_key(), key_prefix(), KeyBucket (+10 more)

### Community 10 - "Community 10"
Cohesion: 0.0
Nodes (34): handle_command(), BotContext, cmd_approvals(), cmd_backups(), cmd_billing(), cmd_config(), cmd_devices(), cmd_emergency() (+26 more)

### Community 11 - "Community 11"
Cohesion: 0.0
Nodes (20): default_max_hosts(), generate_token(), RegisteredAgent, RegisteredClient, Registry, test_deactivate_client(), test_get_agent_by_token(), test_get_client_by_token() (+12 more)

### Community 12 - "Community 12"
Cohesion: 0.0
Nodes (29): AuditFilter, AuditStats, AuditStore, make_event(), make_store(), map_event_to_db_fields(), SiemFormat, test_export_json() (+21 more)

### Community 13 - "Community 13"
Cohesion: 0.0
Nodes (24): AgentInfo, AgentRegistry, ControlPlaneState, deregister_agent(), get_agent(), heartbeat(), HeartbeatRequest, HeartbeatResponse (+16 more)

### Community 14 - "Community 14"
Cohesion: 0.0
Nodes (25): BackendEntry, build_anthropic_body(), build_openai_body(), build_url(), ChatMessage, LlmProxy, LlmRequest, LlmResponse (+17 more)

### Community 15 - "Community 15"
Cohesion: 0.0
Nodes (20): AgentUsage, billing_enforcement_middleware(), extract_tokens_from_payload(), test_active_agents(), test_agent_usage_default(), test_extract_tokens_from_payload(), test_extract_tokens_missing_usage(), test_extract_tokens_partial() (+12 more)

### Community 16 - "Community 16"
Cohesion: 0.0
Nodes (26): RelayHandler, test_agent_lifecycle_connect_send_disconnect(), test_auth_bad_token_returns_none(), test_auth_empty_is_empty(), test_auth_get_client(), test_auth_inactive_client_still_validated(), test_auth_manager_on_handler(), test_auth_register_and_validate() (+18 more)

### Community 17 - "Community 17"
Cohesion: 0.0
Nodes (18): make_user(), RbacError, RbacManager, test_add_and_list_users(), test_command_deny_list(), test_invalid_token(), test_load_users_batch(), test_multi_role_union_permissions() (+10 more)

### Community 18 - "Community 18"
Cohesion: 0.0
Nodes (18): E2eeSessionManager, test_bidirectional_communication(), test_decrypt_from_agent_roundtrip(), test_decrypt_invalid_json_returns_none(), test_decrypt_plaintext_message_returns_none(), test_decrypt_tampered_envelope_fails(), test_encrypt_for_agent_roundtrip(), test_encrypt_no_key_returns_none() (+10 more)

### Community 19 - "Community 19"
Cohesion: 0.0
Nodes (10): extract_account_from_jwt(), get_notifications(), mark_notification_read(), Notification, NotificationStore, build_tls_server_config(), load_certs(), load_key() (+2 more)

### Community 20 - "Community 20"
Cohesion: 0.0
Nodes (18): bench_mcp_parsing(), bench_pool_lookup(), bench_pool_register(), test_agent(), AgentPool, test_agent(), test_concurrent_register_deregister(), test_count() (+10 more)

### Community 21 - "Community 21"
Cohesion: 0.0
Nodes (18): cmd_reload(), ConfigReloader, PushResult, ReloadResult, test_config_json(), test_get_config(), test_push_result_serialization(), test_push_to_nonexistent_agent() (+10 more)

### Community 22 - "Community 22"
Cohesion: 0.0
Nodes (16): build_authn_request(), encode_redirect_request(), extract_all_tags(), extract_attr(), extract_attr_raw(), extract_tag(), generate_request_id(), parse_saml_response() (+8 more)

### Community 23 - "Community 23"
Cohesion: 0.0
Nodes (14): ApprovalDecision, ApprovalQueue, ApprovalRequest, test_approve_responds(), test_concurrent_approve_reject(), test_enqueue_and_list(), test_multiple_pending(), test_reject_responds() (+6 more)

### Community 24 - "Community 24"
Cohesion: 0.0
Nodes (10): RateLimitCategory, RateLimitStats, RateLimitTier, test_check_allows_under_limit(), test_check_blocks_over_limit(), test_check_tiered_pro_higher_limits(), test_check_tiered_uses_plan_limits(), test_cleanup_removes_expired() (+2 more)

### Community 25 - "Community 25"
Cohesion: 0.0
Nodes (8): format_syslog_5424(), RusiemConfig, RusiemEvent, RusiemForwarder, severity_for_action(), test_event(), test_syslog_format(), TestResponse

### Community 26 - "Community 26"
Cohesion: 0.0
Nodes (11): Metrics, metrics_handler(), setup(), test_counter_increment(), test_crypto_metrics(), test_eventbus_counter(), test_false_positives_counter(), test_gauge_set_get() (+3 more)

### Community 27 - "Community 27"
Cohesion: 0.0
Nodes (8): bench_ratelimit(), Bucket, RateLimiter, test_allow_under_limit(), test_block_over_limit(), test_refill_after_time(), test_separate_keys(), shield_ingest_alert()

### Community 28 - "Community 28"
Cohesion: 0.0
Nodes (5): AuthRateLimiter, RateLimitResult, test_allows_under_limit(), test_blocks_over_limit(), test_separate_keys()

### Community 29 - "Community 29"
Cohesion: 0.0
Nodes (9): BotConfig, BotMode, check_bot_health(), Command, run_polling(), run_webhook_handler(), start_polling_mode(), start_tgbot() (+1 more)

### Community 30 - "Community 30"
Cohesion: 0.0
Nodes (0): 

## Knowledge Gaps
- **139 isolated node(s):** `LlmRequest`, `ChatMessage`, `LlmResponse`, `LlmUsage`, `ModelInfo` (+134 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **Thin community `Community 30`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `handle_ws()` connect `Community 18` to `Community 1`, `Community 2`, `Community 4`, `Community 6`, `Community 10`, `Community 15`, `Community 16`, `Community 17`, `Community 20`, `Community 23`?**
  _High betweenness centrality (0.057) - this node is a cross-community bridge._
- **Why does `AuthEngine` connect `Community 3` to `Community 0`, `Community 22`, `Community 7`?**
  _High betweenness centrality (0.028) - this node is a cross-community bridge._
- **Are the 10 inferred relationships involving `build_router()` (e.g. with `spawn_relay()` and `spawn_wss_relay()`) actually correct?**
  _`build_router()` has 10 INFERRED edges - model-reasoned connections that need verification._
- **Are the 26 inferred relationships involving `handle_ws()` (e.g. with `.register_sender()` and `.register()`) actually correct?**
  _`handle_ws()` has 26 INFERRED edges - model-reasoned connections that need verification._
- **Are the 29 inferred relationships involving `handle_command()` (e.g. with `cmd_start()` and `cmd_help()`) actually correct?**
  _`handle_command()` has 29 INFERRED edges - model-reasoned connections that need verification._
- **What connects `LlmRequest`, `ChatMessage`, `LlmResponse` to the rest of the system?**
  _139 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 0` be split into smaller, more focused modules?**
  _Cohesion score 0.03 - nodes in this community are weakly interconnected._