import React from "react";

interface IconProps {
  size?: number;
  className?: string;
  strokeWidth?: number;
}

function I({ size = 24, className = "", strokeWidth = 2, children, ...props }: IconProps & { children: React.ReactNode; [key: string]: unknown }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={strokeWidth} strokeLinecap="round" strokeLinejoin="round" className={className} {...props}>
      {children}
    </svg>
  );
}

export function ShieldIcon(props: IconProps) {
  return (
    <I {...props}>
      <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
    </I>
  );
}

export function ZapIcon(props: IconProps) {
  return (
    <I {...props}>
      <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
    </I>
  );
}

export function LockIcon(props: IconProps) {
  return (
    <I {...props}>
      <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
      <path d="M7 11V7a5 5 0 0 1 10 0v4" />
    </I>
  );
}

export function GitBranchIcon(props: IconProps) {
  return (
    <I {...props}>
      <line x1="6" y1="3" x2="6" y2="15" />
      <circle cx="18" cy="6" r="3" />
      <circle cx="6" cy="18" r="3" />
      <path d="M18 9a9 9 0 0 1-9 9" />
    </I>
  );
}

export function ContainerIcon(props: IconProps) {
  return (
    <I {...props}>
      <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
      <polyline points="22 4 12 14.01 9 11.01" />
    </I>
  );
}

export function ClipboardListIcon(props: IconProps) {
  return (
    <I {...props}>
      <rect x="8" y="2" width="8" height="4" rx="1" ry="1" />
      <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
      <path d="M12 11h4" />
      <path d="M12 16h4" />
      <path d="M8 11h.01" />
      <path d="M8 16h.01" />
    </I>
  );
}

export function TerminalIcon(props: IconProps) {
  return (
    <I {...props}>
      <polyline points="4 17 10 11 4 5" />
      <line x1="12" y1="19" x2="20" y2="19" />
    </I>
  );
}

export function UsersIcon(props: IconProps) {
  return (
    <I {...props}>
      <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
      <circle cx="9" cy="7" r="4" />
      <path d="M23 21v-2a4 4 0 0 0-3-3.87" />
      <path d="M16 3.13a4 4 0 0 1 0 7.75" />
    </I>
  );
}

export function BuildingIcon(props: IconProps) {
  return (
    <I {...props}>
      <rect x="4" y="2" width="16" height="20" rx="2" ry="2" />
      <path d="M9 22v-4h6v4" />
      <path d="M8 6h.01" />
      <path d="M16 6h.01" />
      <path d="M12 6h.01" />
      <path d="M12 10h.01" />
      <path d="M12 14h.01" />
      <path d="M16 10h.01" />
      <path d="M16 14h.01" />
      <path d="M8 10h.01" />
      <path d="M8 14h.01" />
    </I>
  );
}

export function BotIcon(props: IconProps) {
  return (
    <I {...props}>
      <rect x="3" y="11" width="18" height="10" rx="2" />
      <circle cx="12" cy="5" r="2" />
      <path d="M12 7v4" />
      <line x1="8" y1="16" x2="8" y2="16" />
      <line x1="16" y1="16" x2="16" y2="16" />
      <path d="M9 7h6" />
    </I>
  );
}

export function Undo2Icon(props: IconProps) {
  return (
    <I {...props}>
      <polyline points="1 4 1 10 7 10" />
      <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
    </I>
  );
}

export function PlayIcon(props: IconProps) {
  return (
    <I {...props}>
      <polygon points="5 3 19 12 5 21 5 3" />
    </I>
  );
}

export function DatabaseIcon(props: IconProps) {
  return (
    <I {...props}>
      <ellipse cx="12" cy="5" rx="9" ry="3" />
      <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3" />
      <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5" />
    </I>
  );
}

export function CodeIcon(props: IconProps) {
  return (
    <I {...props}>
      <polyline points="16 18 22 12 16 6" />
      <polyline points="8 6 2 12 8 18" />
    </I>
  );
}

export function AlertTriangleIcon(props: IconProps) {
  return (
    <I {...props}>
      <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
      <line x1="12" y1="9" x2="12" y2="13" />
      <line x1="12" y1="17" x2="12.01" y2="17" />
    </I>
  );
}

export function CheckCircleIcon(props: IconProps) {
  return (
    <I {...props}>
      <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
      <polyline points="22 4 12 14.01 9 11.01" />
    </I>
  );
}

export function XCircleIcon(props: IconProps) {
  return (
    <I {...props}>
      <circle cx="12" cy="12" r="10" />
      <line x1="15" y1="9" x2="9" y2="15" />
      <line x1="9" y1="9" x2="15" y2="15" />
    </I>
  );
}

export function ClockIcon(props: IconProps) {
  return (
    <I {...props}>
      <circle cx="12" cy="12" r="10" />
      <polyline points="12 6 12 12 16 14" />
    </I>
  );
}

export function ArrowRightIcon(props: IconProps) {
  return (
    <I {...props}>
      <line x1="5" y1="12" x2="19" y2="12" />
      <polyline points="12 5 19 12 12 19" />
    </I>
  );
}

export function ServerIcon(props: IconProps) {
  return (
    <I {...props}>
      <rect x="2" y="2" width="20" height="8" rx="2" ry="2" />
      <rect x="2" y="14" width="20" height="8" rx="2" ry="2" />
      <line x1="6" y1="6" x2="6.01" y2="6" />
      <line x1="6" y1="18" x2="6.01" y2="18" />
    </I>
  );
}
