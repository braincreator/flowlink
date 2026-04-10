import React from "react";

interface FlowLinkLogoProps {
  size?: number;
  className?: string;
  showText?: boolean;
}

export function FlowLinkLogo({ size = 32, className = "", showText = true }: FlowLinkLogoProps) {
  return (
    <span className={`flowlink-logo ${className}`} style={{ display: "inline-flex", alignItems: "center", gap: "10px" }}>
      <svg width={size} height={size} viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <linearGradient id="logoArcGrad" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#1e40af" />
            <stop offset="50%" stopColor="#3b82f6" />
            <stop offset="100%" stopColor="#60a5fa" />
          </linearGradient>
        </defs>
        <circle cx="50" cy="50" r="6" fill="#3b82f6" />
        <circle cx="50" cy="50" r="18" fill="none" stroke="#93c5fd" strokeWidth="5" strokeLinecap="round"
          strokeDasharray="70 113" transform="rotate(-110 50 50)" />
        <circle cx="50" cy="50" r="32" fill="none" stroke="url(#logoArcGrad)" strokeWidth="5" strokeLinecap="round"
          strokeDasharray="110 201" transform="rotate(-100 50 50)" />
      </svg>
      {showText && (
        <span style={{ fontSize: "18px", fontWeight: 700, letterSpacing: "-0.02em", color: "var(--text-primary)" }}>
          flow<span style={{ color: "var(--color-primary-light)" }}>link</span>
        </span>
      )}
    </span>
  );
}
