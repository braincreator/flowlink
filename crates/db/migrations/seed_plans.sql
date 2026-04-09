-- Seed plans matching landing page prices
-- Run this on the VPS after migration 008_plans is applied

-- Cloud Starter
INSERT INTO plans (id, name, description, tier, price_kopecks, annual_price_kopecks, period, currency, limits, features, is_active, sort_order)
VALUES (
    'starter',
    'Cloud Starter',
    'Для индивидуальных пользователей и небольших проектов',
    1,
    199900,
    1999000,
    'month',
    'RUB',
    '{
        "api_requests_per_day": 1000,
        "tokens_per_day": 500000,
        "max_agents": 3,
        "storage_mb": 2048,
        "max_payload_kb": 1024,
        "max_agents_total": 5,
        "webhook_rate_per_min": 30,
        "mcp_tools_per_agent": 10,
        "audit_retention_days": 30,
        "priority_support": false,
        "custom_domain": false
    }'::jsonb,
    '["3 агента", "1K запросов/день", "500K токенов/день", "2 GB хранилище", "10 MCP инструментов", "30 дней аудита", "Email поддержка"]'::jsonb,
    true,
    1
) ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    price_kopecks = EXCLUDED.price_kopecks,
    limits = EXCLUDED.limits,
    features = EXCLUDED.features;

-- Cloud Pro
INSERT INTO plans (id, name, description, tier, price_kopecks, annual_price_kopecks, period, currency, limits, features, is_active, sort_order)
VALUES (
    'pro',
    'Cloud Pro',
    'Для продвинутых пользователей и малого бизнеса',
    2,
    499900,
    4999000,
    'month',
    'RUB',
    '{
        "api_requests_per_day": 10000,
        "tokens_per_day": 5000000,
        "max_agents": 10,
        "storage_mb": 10240,
        "max_payload_kb": 5120,
        "max_agents_total": 25,
        "webhook_rate_per_min": 100,
        "mcp_tools_per_agent": 50,
        "audit_retention_days": 90,
        "priority_support": true,
        "custom_domain": false
    }'::jsonb,
    '["10 агентов", "10K запросов/день", "5M токенов/день", "10 GB хранилище", "50 MCP инструментов", "Приоритетная поддержка", "90 дней аудита", "Shield защита", "ServerGuard мониторинг"]'::jsonb,
    true,
    2
) ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    price_kopecks = EXCLUDED.price_kopecks,
    limits = EXCLUDED.limits,
    features = EXCLUDED.features;

-- Free (always last in sort, but tier 0)
INSERT INTO plans (id, name, description, tier, price_kopecks, annual_price_kopecks, period, currency, limits, features, is_active, sort_order)
VALUES (
    'free',
    'Free',
    'Для знакомства с FlowLink',
    0,
    0,
    NULL,
    'month',
    'RUB',
    '{
        "api_requests_per_day": 100,
        "tokens_per_day": 50000,
        "max_agents": 1,
        "storage_mb": 100,
        "max_payload_kb": 512,
        "max_agents_total": 1,
        "webhook_rate_per_min": 10,
        "mcp_tools_per_agent": 5,
        "audit_retention_days": 7,
        "priority_support": false,
        "custom_domain": false
    }'::jsonb,
    '["1 агент", "100 запросов/день", "50K токенов/день", "100 MB хранилище", "5 MCP инструментов", "Базовый мониторинг"]'::jsonb,
    true,
    0
) ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    price_kopecks = EXCLUDED.price_kopecks,
    limits = EXCLUDED.limits,
    features = EXCLUDED.features;
