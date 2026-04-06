import React from "react";
import {
  AbsoluteFill,
  useCurrentFrame,
  useVideoConfig,
  interpolate,
  spring,
} from "remotion";

// ── Shared config ──
const FPS = 30;
const WIDTH = 960;
const HEIGHT = 540;

type LineType = "prompt" | "warning" | "info" | "success" | "output" | "blank" | "error" | "typing";

interface TermLine {
  type: LineType;
  text: string;
  startFrame: number;
  duration: number;
}

const COLORS = {
  bg: "transparent",
  cardBg: "#0f0f12",
  border: "rgba(255,255,255,0.08)",
  textPrimary: "#EDEDEF",
  textMuted: "#6B7280",
  accent: "#22C55E",
  accentDim: "rgba(34,197,94,0.15)",
  warning: "#F59E0B",
  warningBg: "rgba(245,158,11,0.06)",
  error: "#EF4444",
  errorBg: "rgba(239,68,68,0.06)",
  info: "#3B82F6",
  infoBg: "rgba(59,130,246,0.06)",
  prompt: "#22C55E",
  cmd: "#EDEDEF",
  output: "#6B7280",
  success: "#22C55E",
};

// ── 4 Case Studies ──

export const CASE_DROP_TABLE = {
  id: "CaseDropTable",
  title: "DROP TABLE",
  fps: FPS,
  width: WIDTH,
  height: HEIGHT,
  durationInFrames: 360,
  topbarTitle: "flowlink@prod ~ psql",
  lines: [
    { type: "output" as LineType, text: "-- connecting to production database...", startFrame: 10, duration: 10 },
    { type: "typing" as LineType, text: "psql> DROP TABLE users CASCADE;", startFrame: 30, duration: 40 },
    { type: "warning" as LineType, text: "⚠ FlowLink: data_destroy — DROP TABLE detected (3 tables, 1.2GB)", startFrame: 80, duration: 12 },
    { type: "info" as LineType, text: "📦 Snapshot: db_users_1709876543.sql.gz (340KB)", startFrame: 100, duration: 15 },
    { type: "success" as LineType, text: "✓ Команда выполнена — 3 tables dropped", startFrame: 125, duration: 8 },
    { type: "blank" as LineType, text: "", startFrame: 145, duration: 1 },
    { type: "error" as LineType, text: "🚨 Slack: @channel — прод бд мертва, авторег не работает!", startFrame: 150, duration: 12 },
    { type: "blank" as LineType, text: "", startFrame: 172, duration: 1 },
    { type: "typing" as LineType, text: "psql> flowlink undo db_users_1709876543", startFrame: 180, duration: 35 },
    { type: "info" as LineType, text: "📦 Restoring: users, sessions, preferences...", startFrame: 225, duration: 15 },
    { type: "success" as LineType, text: "✓ Восстановлено за 0.8s — 48,200 rows на месте", startFrame: 248, duration: 12 },
    { type: "blank" as LineType, text: "", startFrame: 270, duration: 1 },
  ],
};

export const CASE_DOCKER_RM = {
  id: "CaseDockerRm",
  title: "Docker Disaster",
  fps: FPS,
  width: WIDTH,
  height: HEIGHT,
  durationInFrames: 360,
  topbarTitle: "flowlink@prod ~ docker",
  lines: [
    { type: "output" as LineType, text: "-- cleaning up old containers...", startFrame: 10, duration: 10 },
    { type: "typing" as LineType, text: "$ docker rm -f $(docker ps -aq)", startFrame: 30, duration: 38 },
    { type: "warning" as LineType, text: "⚠ FlowLink: service_disrupt — docker rm ALL containers (7 active)", startFrame: 78, duration: 12 },
    { type: "info" as LineType, text: "📦 Snapshot: docker_compose_state_1709876543.tar.gz (2.1MB)", startFrame: 98, duration: 15 },
    { type: "success" as LineType, text: "✓ Команда выполнена — 7 containers removed", startFrame: 123, duration: 8 },
    { type: "blank" as LineType, text: "", startFrame: 143, duration: 1 },
    { type: "error" as LineType, text: "🚨 Grafana: API unavailable, PostgreSQL: connection refused", startFrame: 148, duration: 12 },
    { type: "blank" as LineType, text: "", startFrame: 170, duration: 1 },
    { type: "typing" as LineType, text: "$ flowlink undo docker_compose_state_1709876543", startFrame: 178, duration: 38 },
    { type: "info" as LineType, text: "📦 Restoring containers: api, db, redis, nginx, worker, cron, monitor...", startFrame: 226, duration: 18 },
    { type: "success" as LineType, text: "✓ Восстановлено за 2.1s — 7 containers running", startFrame: 254, duration: 12 },
    { type: "blank" as LineType, text: "", startFrame: 276, duration: 1 },
  ],
};

export const CASE_GIT_RESET = {
  id: "CaseGitReset",
  title: "Git Hard Reset",
  fps: FPS,
  width: WIDTH,
  height: HEIGHT,
  durationInFrames: 360,
  topbarTitle: "flowlink@prod ~/app",
  lines: [
    { type: "output" as LineType, text: "-- reverting bad deploy...", startFrame: 10, duration: 10 },
    { type: "typing" as LineType, text: "$ git reset --hard HEAD~10", startFrame: 30, duration: 35 },
    { type: "warning" as LineType, text: "⚠ FlowLink: data_destroy — git reset --hard (10 commits, 47 files)", startFrame: 75, duration: 12 },
    { type: "info" as LineType, text: "📦 Snapshot: git_state_1709876543.tar.gz (890KB, 47 files)", startFrame: 95, duration: 15 },
    { type: "success" as LineType, text: "✓ HEAD is now at a3f8c2d — 10 commits removed", startFrame: 120, duration: 8 },
    { type: "blank" as LineType, text: "", startFrame: 140, duration: 1 },
    { type: "error" as LineType, text: "🚨 GitHub: pushed 3 of those commits — remote out of sync!", startFrame: 145, duration: 12 },
    { type: "blank" as LineType, text: "", startFrame: 167, duration: 1 },
    { type: "typing" as LineType, text: "$ flowlink undo git_state_1709876543", startFrame: 175, duration: 35 },
    { type: "info" as LineType, text: "📦 Restoring: .git/objects, working tree, staging area...", startFrame: 220, duration: 15 },
    { type: "success" as LineType, text: "✓ Восстановлено за 0.4s — 10 commits + 47 files на месте", startFrame: 245, duration: 12 },
    { type: "blank" as LineType, text: "", startFrame: 267, duration: 1 },
  ],
};

export const CASE_CHMOD_777 = {
  id: "CaseChmod777",
  title: "chmod 777",
  fps: FPS,
  width: WIDTH,
  height: HEIGHT,
  durationInFrames: 360,
  topbarTitle: "flowlink@prod /etc",
  lines: [
    { type: "output" as LineType, text: "-- fixing permissions issue...", startFrame: 10, duration: 10 },
    { type: "typing" as LineType, text: "$ chmod -R 777 /etc/ssl /etc/ssh", startFrame: 30, duration: 40 },
    { type: "warning" as LineType, text: "⚠ FlowLink: security_bypass — chmod 777 on system directories", startFrame: 80, duration: 12 },
    { type: "info" as LineType, text: "📦 Snapshot: permissions_ssl_ssh_1709876543.tar.gz (24KB)", startFrame: 100, duration: 15 },
    { type: "success" as LineType, text: "✓ Permissions changed — 156 files modified", startFrame: 125, duration: 8 },
    { type: "blank" as LineType, text: "", startFrame: 145, duration: 1 },
    { type: "error" as LineType, text: "🚨 fail2ban: /etc/ssl/private readable by all — SSH brute force detected!", startFrame: 150, duration: 14 },
    { type: "blank" as LineType, text: "", startFrame: 174, duration: 1 },
    { type: "typing" as LineType, text: "$ flowlink undo permissions_ssl_ssh_1709876543", startFrame: 182, duration: 38 },
    { type: "info" as LineType, text: "📦 Restoring: /etc/ssl (89 files), /etc/ssh (67 files)...", startFrame: 230, duration: 15 },
    { type: "success" as LineType, text: "✓ Восстановлено за 0.2s — permissions + ACLs на месте", startFrame: 255, duration: 12 },
    { type: "blank" as LineType, text: "", startFrame: 277, duration: 1 },
  ],
};

// ── Shared Terminal Component ──

const TerminalLine: React.FC<{ line: TermLine; frame: number; fps: number }> = ({
  line,
  frame,
  fps,
}) => {
  const relativeFrame = frame - line.startFrame;

  if (relativeFrame < 0) return null;

  if (line.type === "blank") return <div style={{ height: 20 }} />;

  const isTyping = line.type === "typing";

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
    const springVal = spring({
      frame: relativeFrame,
      fps,
      config: { damping: 30, stiffness: 200, mass: 0.8 },
    });
    opacity = Math.max(0, springVal);
    visibleChars = line.text.length;
  }

  const displayText = line.text.slice(0, visibleChars);

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
      bgColor = COLORS.infoBg;
      break;
    case "success":
      color = COLORS.success;
      break;
    case "error":
      color = COLORS.error;
      bgColor = COLORS.errorBg;
      break;
    case "output":
      color = COLORS.output;
      break;
  }

  // Color the prompt part
  const isPromptLine = line.type === "typing" && displayText.startsWith("$ ");
  const isPsqlLine = line.type === "typing" && displayText.startsWith("psql> ");
  let promptPart = "";
  let cmdPart = displayText;

  if (isPromptLine) {
    promptPart = "$ ";
    cmdPart = displayText.slice(2);
  } else if (isPsqlLine) {
    promptPart = "psql> ";
    cmdPart = displayText.slice(6);
  }

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
        fontSize: 15,
        lineHeight: 1.5,
        letterSpacing: "-0.01em",
        whiteSpace: "nowrap",
        overflow: "hidden",
      }}
    >
      {promptPart && (
        <span style={{ color: COLORS.prompt, flexShrink: 0 }}>{promptPart}</span>
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
            opacity: frame % 30 < 15 ? 1 : 0,
          }}
        />
      )}
    </div>
  );
};

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
        fontSize: 15,
      }}
    >
      <span style={{ color: COLORS.prompt }}>$ </span>
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

const TerminalTopbar: React.FC<{ title: string }> = ({ title }) => (
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
      {title}
    </span>
  </div>
);

// ── Generic Case Study Component ──

interface CaseStudyConfig {
  id: string;
  title: string;
  fps: number;
  width: number;
  height: number;
  durationInFrames: number;
  topbarTitle: string;
  lines: TermLine[];
}

const CaseStudyTerminal: React.FC<{ config: CaseStudyConfig }> = ({ config }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const glowOpacity = interpolate(Math.sin(frame * 0.02), [-1, 1], [0.08, 0.15]);

  const cardEntrance = spring({
    frame,
    fps,
    config: { damping: 30, stiffness: 80, mass: 1.0 },
  });

  const cardY = interpolate(cardEntrance, [0, 1], [30, 0]);
  const cardOpacity = Math.max(0, cardEntrance);

  const lastLine = config.lines[config.lines.length - 1];
  const cursorVisible = frame > lastLine.startFrame + 30;

  const fadeOutStart = config.durationInFrames - 20;
  const globalOpacity = interpolate(frame, [fadeOutStart, config.durationInFrames], [1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill style={{ background: COLORS.bg, display: "flex", alignItems: "center", justifyContent: "center" }}>
      <div
        style={{
          opacity: globalOpacity * cardOpacity,
          transform: `translateY(${cardY}px)`,
          width: config.width - 60,
          maxWidth: "100%",
          borderRadius: 16,
          overflow: "hidden",
          border: `1px solid ${COLORS.border}`,
          background: COLORS.cardBg,
          position: "relative",
        }}
      >
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

        <TerminalTopbar title={config.topbarTitle} />

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
          {config.lines.map((line, i) => (
            <TerminalLine key={i} line={line} frame={frame} fps={fps} />
          ))}
          <BlinkingCursor frame={frame} visible={cursorVisible} />
        </div>
      </div>
    </AbsoluteFill>
  );
};

// ── Exported components ──

export const CaseDropTable: React.FC = () => <CaseStudyTerminal config={CASE_DROP_TABLE} />;
export const CaseDockerRm: React.FC = () => <CaseStudyTerminal config={CASE_DOCKER_RM} />;
export const CaseGitReset: React.FC = () => <CaseStudyTerminal config={CASE_GIT_RESET} />;
export const CaseChmod777: React.FC = () => <CaseStudyTerminal config={CASE_CHMOD_777} />;
