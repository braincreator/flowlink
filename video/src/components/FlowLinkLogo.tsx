import React from "react";

interface FlowLinkLogoProps {
  size?: number;
  style?: React.CSSProperties;
}

/**
 * FlowLink Logo — exact replica of the website logo.
 * Uses <path> arcs instead of circle+strokeDasharray for reliable rendering.
 *
 * Inner arc: r=18, ~223° sweep, light blue (#93c5fd)
 * Outer arc: r=32, ~197° sweep, gradient blue (#1e40af → #3b82f6 → #60a5fa)
 * Center dot: r=6, solid blue (#3b82f6)
 */
export const FlowLinkLogo: React.FC<FlowLinkLogoProps> = ({ size = 80, style }) => {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      style={style}
    >
      <defs>
        <linearGradient id="flArcGrad" x1="0%" y1="0%" x2="100%" y2="0%">
          <stop offset="0%" stopColor="#1e40af" />
          <stop offset="50%" stopColor="#3b82f6" />
          <stop offset="100%" stopColor="#60a5fa" />
        </linearGradient>
      </defs>

      {/* Center dot */}
      <circle cx="50" cy="50" r="6" fill="#3b82f6" />

      {/* Inner arc — ~223° sweep, light blue */}
      <path
        d="M 43.84 33.09 A 18 18 0 1 1 43.02 66.59"
        stroke="#93c5fd"
        strokeWidth="5"
        strokeLinecap="round"
        fill="none"
      />

      {/* Outer arc — ~197° sweep, gradient */}
      <path
        d="M 44.44 18.49 A 32 32 0 1 1 46.13 81.76"
        stroke="url(#flArcGrad)"
        strokeWidth="5"
        strokeLinecap="round"
        fill="none"
      />
    </svg>
  );
};
