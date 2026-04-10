import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Server mode — enables API routes, SSR, middleware
  // Previously was "export" (static only), now needs BFF proxy to relay
  images: {
    unoptimized: true,
  },
  // Allow relay API connections from website BFF
  async rewrites() {
    const relayUrl = process.env.RELAY_URL;
    if (!relayUrl) return [];
    return [
      {
        source: "/api/relay/:path*",
        destination: `${relayUrl}/api/:path*`,
      },
      {
        source: "/healthz",
        destination: `${relayUrl}/healthz`,
      },
    ];
  },
};

export default nextConfig;
