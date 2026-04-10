"use client";

import React, { useState, useEffect, useRef } from "react";

interface DemoStep {
  type: "prompt" | "warning" | "success" | "error" | "info" | "blank";
  text: string;
}

const DEMO_STEPS: DemoStep[] = [
  { type: "prompt", text: "$ claude \"optimize database\"" },
  { type: "blank", text: "" },
  { type: "warning", text: "  DETECTED: DROP TABLE pattern found in generated SQL" },
  { type: "info", text: "  Analyzing AST tree... 3 destructive operations identified" },
  { type: "blank", text: "" },
  { type: "success", text: "  Snapshot: 3 files (12KB) created in 2ms" },
  { type: "info", text: "  Backup: /var/lib/postgresql/data/backup_20260409.sql.gz" },
  { type: "blank", text: "" },
  { type: "error", text: "  BLOCKED - risk score: 9/10" },
  { type: "info", text: "  Reason: DROP TABLE without WHERE clause (irreversible)" },
  { type: "info", text: "  Action required: manual approval or whitelist rule" },
  { type: "blank", text: "" },
  { type: "prompt", text: "$ flowlink undo --last" },
  { type: "blank", text: "" },
  { type: "success", text: "  Restored 3 files from snapshot in 45ms" },
  { type: "success", text: "  Database state: consistent (pre-block state)" },
  { type: "success", text: "  Server status: healthy" },
];

const STEP_DELAY = 400;

export function DemoVideo() {
  const [visibleSteps, setVisibleSteps] = useState<number>(0);
  const [isRunning, setIsRunning] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const [started, setStarted] = useState(false);

  const startAnimation = () => {
    if (started) return;
    setStarted(true);
    setVisibleSteps(0);
    setIsRunning(true);
  };

  useEffect(() => {
    if (!isRunning) return;
    if (visibleSteps >= DEMO_STEPS.length) {
      setIsRunning(false);
      return;
    }

    const timer = setTimeout(() => {
      setVisibleSteps((v) => v + 1);
    }, STEP_DELAY);

    return () => clearTimeout(timer);
  }, [isRunning, visibleSteps]);

  useEffect(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [visibleSteps]);

  const getLineColor = (type: string): string => {
    switch (type) {
      case "prompt": return "var(--text-primary)";
      case "warning": return "#f59e0b";
      case "success": return "#22c55e";
      case "error": return "#ef4444";
      case "info": return "var(--color-primary-light)";
      default: return "transparent";
    }
  };

  return (
    <div className="demo-terminal">
      <div className="demo-terminal-topbar">
        <div className="demo-terminal-dots">
          <span className="demo-dot demo-dot-red" />
          <span className="demo-dot demo-dot-yellow" />
          <span className="demo-dot demo-dot-green" />
        </div>
        <span className="demo-terminal-title">flowlink@server ~ bash</span>
        <div style={{ width: 52 }} />
      </div>
      <div className="demo-terminal-body" ref={containerRef}>
        {!started ? (
          <div className="demo-terminal-start" onClick={startAnimation}>
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--color-primary-light)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <polygon points="5 3 19 12 5 21 5 3" />
            </svg>
            <span>Запустить демо</span>
          </div>
        ) : (
          <>
            {DEMO_STEPS.slice(0, visibleSteps).map((step, i) => (
              <div key={i} className="demo-line" style={{ color: getLineColor(step.type) }}>
                {step.text || "\u00A0"}
              </div>
            ))}
            {isRunning && (
              <div className="demo-line" style={{ color: "var(--color-primary-light)" }}>
                <span className="demo-cursor" />
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
