import React from "react";

interface FlowLinkLogoProps {
  size?: number;
  style?: React.CSSProperties;
}

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
      <circle cx="50" cy="50" r="6" fill="#3b82f6" />
      <circle cx="50" cy="50" r="18" fill="none" stroke="#93c5fd" strokeWidth="5" strokeLinecap="round"
        strokeDasharray="70 113" transform="rotate(-110 50 50)" />
      <circle cx="50" cy="50" r="32" fill="none" stroke="url(#flArcGrad)" strokeWidth="5" strokeLinecap="round"
        strokeDasharray="110 201" transform="rotate(-100 50 50)" />
    </svg>
  );
};
