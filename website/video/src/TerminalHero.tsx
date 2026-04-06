import React from "react";
import {
  AbsoluteFill,
  useCurrentFrame,
  useVideoConfig,
  interpolate,
  spring,
  Easing,
} from "remotion";

export const TERMINAL_HERO_CONFIG = {
  fps: 30,
  width: 960,
  height: 540,
  durationInFrames: 360, // 12 seconds — loops nicely
};

// ── Color palette matching the landing page ──
const COLORS = {
  bg: "#0a0a0c",
  cardBg: "#0f0f12",
  border: "rgba(255,255,255,0.08)",
  textPrimary: "#EDEDEF",
  textMuted: "#6B7280",
  accent: "#22C55E",
  accentDim: "rgba(34,197,94,0.15)",
  warning: "#F59E0B",
  warningBg: "rgba(245,158,11,0.06)",
  error: "#EF4444",
  info: "#3B82F6",
  prompt: "#22C55E",
  cmd: "#EDEDEF",
  output: "#6B7280",
  success: "#22C55E",
};

// ── Terminal line types ──
type LineType = "prompt" | "warning" | "info" | "success" | "output" | "blank" | "typing";

interface TermLine {
  type: LineType;
  text: string;
  /** frame when this line starts appearing */
  startFrame: number;
  /** frames to type out (for typing lines) or appear (for instant lines) */
  duration: number;
}

// ── Timeline: what happens when ──
const LINES: TermLine[] = [
  // Phase 1: Dangerous command
  {
    type: "typing",
    text: "$ rm -rf /var/www/config/",
    startFrame: 15,
    duration: 45, // type over 1.5s
  },
  // Phase 2: FlowLink warning
  {
    type: "warning",
    text: "⚠ FlowLink: опасная команда обнаружена",
    startFrame: 70,
    duration: 12,
  },
  // Phase 3: Snapshot creation
  {
    type: "info",
    text: "📦 Snapshot: config_backup_1712345678.tar.gz (12KB, 14 файлов)",
    startFrame: 90,
    duration: 15,
  },
  // Phase 4: Command executed
  {
    type: "success",
    text: "✓ Команда выполнена",
    startFrame: 115,
    duration: 8,
  },
  // blank line
  {
    type: "blank",
    text: "",
    startFrame: 135,
    duration: 1,
  },
  // Phase 5: Realization
  {
    type: "output",
    text: "...позже понял что нужно вернуть",
    startFrame: 140,
    duration: 10,
  },
  // blank
  {
    type: "blank",
    text: "",
    startFrame: 155,
    duration: 1,
  },
  // Phase 6: Undo command
  {
    type: "typing",
    text: "$ flowlink undo config_backup_1712345678",
    startFrame: 165,
    duration: 40,
  },
  // Phase 7: Restore success
  {
    type: "success",
    text: "✓ Восстановлено за 0.3s — 14 файлов на месте",
    startFrame: 215,
    duration: 12,
  },
  // blank — pause before loop
  {
    type: "blank",
    text: "",
    startFrame: 235,
    duration: 1,
  },
  // Blinking cursor at the end
  {
    type: "blank",
    text: "",
    startFrame: 250,
    duration: 1,
  },
];

// ── Single terminal line component ──
const TerminalLine: React.FC<{ line: TermLine; frame: number; fps: number }> = ({
  line,
  frame,
  fps,
}) => {
  const relativeFrame = frame - line.startFrame;

  if (relativeFrame < 0) return null;

  if (line.type === "blank") return <div style={{ height: 20 }} />;

  const isTyping = line.type === "typing";

  // How much of the text is visible
  let visibleChars: number;
  let opacity: number;

  if (isTyping) {
    const typeProgress = interpolate(relativeFrame, [0, line.duration], [0, line.text.length], {
      extrapolateLeft: "clamp",
      extrapolateRight: "clamp",
    });
    visibleChars = Math.round(typeProgress);
    opacity = 1;
  } else {
    // Instant appearance with spring fade
    const springVal = spring({
      frame: relativeFrame,
      fps,
      config: { damping: 30, stiffness: 200, mass: 0.8 },
    });
    opacity = Math.max(0, springVal);
    visibleChars = line.text.length;
  }

  const displayText = line.text.slice(0, visibleChars);

  // Color based on type
  let color = COLORS.output;
  let bgColor: string | undefined;

  switch (line.type) {
    case "typing":
      color = COLORS.cmd;
      break;
    case "warning":
      color = COLORS.warning;
      bgColor = COLORS.warningBg;
      break;
    case "info":
      color = COLORS.info;
      break;
    case "success":
      color = COLORS.success;
      break;
    case "output":
      color = COLORS.output;
      break;
  }

  // For typing lines, color the $ prompt green
  const isPromptLine = line.type === "typing" && displayText.startsWith("$");
  const promptPart = isPromptLine ? "$ " : "";
  const cmdPart = isPromptLine ? displayText.slice(2) : displayText;

  return (
    <div
      style={{
        opacity,
        height: 32,
        display: "flex",
        alignItems: "center",
        paddingLeft: 8,
        paddingRight: 8,
        borderRadius: 4,
        background: bgColor,
        fontFamily: "'JetBrains Mono', 'SF Mono', 'Fira Code', Menlo, monospace",
        fontSize: 16,
        lineHeight: 1.5,
        letterSpacing: "-0.01em",
        whiteSpace: "nowrap",
        overflow: "hidden",
      }}
    >
      {isPromptLine && (
        <span style={{ color: COLORS.prompt, marginRight: 0, flexShrink: 0 }}>{promptPart}</span>
      )}
      <span style={{ color }}>{cmdPart}</span>
      {isTyping && visibleChars < line.text.length && (
        <span
          style={{
            display: "inline-block",
            width: 9,
            height: 20,
            background: COLORS.accent,
            marginLeft: 2,
            borderRadius: 1,
            opacity: frame % 30 < 15 ? 1 : 0, // blink at 1Hz
          }}
        />
      )}
    </div>
  );
};

// ── Blinking cursor (shown after all lines) ──
const BlinkingCursor: React.FC<{ frame: number; visible: boolean }> = ({ frame, visible }) => {
  if (!visible) return null;
  const show = frame % 30 < 18;
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        paddingLeft: 8,
        height: 32,
        fontFamily: "'JetBrains Mono', 'SF Mono', monospace",
        fontSize: 16,
      }}
    >
      <span style={{ color: COLORS.prompt, marginRight: 0 }}>$ </span>
      {show && (
        <span
          style={{
            display: "inline-block",
            width: 9,
            height: 20,
            background: COLORS.accent,
            marginLeft: 2,
            borderRadius: 1,
          }}
        />
      )}
    </div>
  );
};

// ── Topbar (macOS dots) ──
const TerminalTopbar: React.FC = () => (
  <div
    style={{
      display: "flex",
      alignItems: "center",
      gap: 8,
      padding: "14px 20px",
      borderBottom: `1px solid ${COLORS.border}`,
      background: "rgba(255,255,255,0.02)",
    }}
  >
    <span style={{ width: 12, height: 12, borderRadius: "50%", background: "#ef4444" }} />
    <span style={{ width: 12, height: 12, borderRadius: "50%", background: "#f59e0b" }} />
    <span style={{ width: 12, height: 12, borderRadius: "50%", background: "#22c55e" }} />
    <span
      style={{
        flex: 1,
        textAlign: "center",
        fontSize: 13,
        color: COLORS.textMuted,
        fontFamily: "'JetBrains Mono', monospace",
      }}
    >
      flowlink@prod ~
    </span>
  </div>
);

// ── Main component ──
export const TerminalHero: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Subtle ambient glow that pulses
  const glowOpacity = interpolate(Math.sin(frame * 0.02), [-1, 1], [0.08, 0.15]);

  // Card entrance animation
  const cardEntrance = spring({
    frame,
    fps,
    config: { damping: 30, stiffness: 80, mass: 1.0 },
  });

  const cardY = interpolate(cardEntrance, [0, 1], [30, 0]);
  const cardOpacity = Math.max(0, cardEntrance);

  // Determine if cursor should show (after all lines are done typing)
  const lastLine = LINES[LINES.length - 1];
  const cursorVisible = frame > lastLine.startFrame + 30;

  // Fade out near the end for smooth loop
  const fadeOutStart = TERMINAL_HERO_CONFIG.durationInFrames - 20;
  const globalOpacity = interpolate(frame, [fadeOutStart, TERMINAL_HERO_CONFIG.durationInFrames], [1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill
      style={{
        background: "transparent",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <div
        style={{
          opacity: globalOpacity * cardOpacity,
          transform: `translateY(${cardY}px)`,
          width: TERMINAL_HERO_CONFIG.width - 60,
          maxWidth: "100%",
          borderRadius: 16,
          overflow: "hidden",
          border: `1px solid ${COLORS.border}`,
          background: COLORS.cardBg,
          position: "relative",
        }}
      >
        {/* Ambient glow behind card */}
        <div
          style={{
            position: "absolute",
            top: -60,
            left: "50%",
            transform: "translateX(-50%)",
            width: 400,
            height: 250,
            background: `radial-gradient(ellipse, rgba(34,197,94,${glowOpacity}) 0%, transparent 70%)`,
            pointerEvents: "none",
            zIndex: 0,
          }}
        />

        <TerminalTopbar />

        <div
          style={{
            padding: "20px 24px",
            display: "flex",
            flexDirection: "column",
            gap: 4,
            position: "relative",
            zIndex: 1,
          }}
        >
          {LINES.map((line, i) => (
            <TerminalLine key={i} line={line} frame={frame} fps={fps} />
          ))}
          <BlinkingCursor frame={frame} visible={cursorVisible} />
        </div>
      </div>
    </AbsoluteFill>
  );
};
