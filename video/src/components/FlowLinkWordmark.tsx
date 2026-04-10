import React from "react";

interface FlowLinkWordmarkProps {
  size?: number;
  style?: React.CSSProperties;
}

export const FlowLinkWordmark: React.FC<FlowLinkWordmarkProps> = ({ size = 48, style }) => {
  return (
    <span style={{
      fontSize: size,
      fontWeight: 800,
      letterSpacing: "-0.03em",
      color: "#ffffff",
      fontFamily: "system-ui, -apple-system, sans-serif",
      ...style,
    }}>
      flow<span style={{ color: "#3b82f6" }}>link</span>
    </span>
  );
};
