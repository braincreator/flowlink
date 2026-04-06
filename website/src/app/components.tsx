"use client";

export function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = React.useState(false);

  return (
    <button
      className="copy-btn"
      onClick={() => {
        navigator.clipboard.writeText(text);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      }}
    >
      {copied ? "✓ Copied" : "Copy"}
    </button>
  );
}

import React from "react";
