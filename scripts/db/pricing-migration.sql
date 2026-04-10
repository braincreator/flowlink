-- FlowLink Pricing Migration: Normalize all plans to identical features
-- This aligns database schema with "resource gating, not feature gating" strategy
-- Run after billing crate update to ensure consistent architecture

-- First, let's see current data
SELECT 
    id, 
    name,
    features,
    price_kopecks,
    annual_price_kopecks
FROM plans 
ORDER BY id;

-- Update all plans to have identical features array (same as Rust plans.rs)
-- Features are now identical across all plans — only limits differ
UPDATE plans 
SET 
    features = '[
        "Pattern blocking",
        "AST-анализ обфускации",
        "E2EE шифрование",
        "Telegram бот",
        "Web dashboard",
        "Device trust",
        "MCP protocol",
        "Audit log + HMAC"
    ]',
    updated_at = NOW()
WHERE id IN ('trial', 'starter', 'pro');

-- Verify update
SELECT id, name, features FROM plans WHERE id IN ('trial', 'starter', 'pro');

-- Also set proper pricing (RUB, kopecks)
UPDATE plans 
SET 
    price_kopecks = CASE id 
        WHEN 'trial' THEN 0 
        WHEN 'starter' THEN 199000 
        WHEN 'pro' THEN 599000
    END,
    annual_price_kopecks = CASE id 
        WHEN 'trial' THEN NULL
        WHEN 'starter' THEN 1910400
        WHEN 'pro' THEN 5750800
    END,
    currency = 'RUB',
    period = 'month',
    updated_at = NOW()
WHERE id IN ('trial', 'starter', 'pro');

-- Final verification
SELECT 
    id,
    name,
    price_kopecks,
    annual_price_kopecks,
    features,
    limits
FROM plans 
ORDER BY sort_order;
