import type { NextConfig } from "next";

/**
 * Headers that do not vary per request. The Content-Security-Policy is NOT
 * here: it carries a per-request nonce and is set in src/middleware.ts.
 */
const SECURITY_HEADERS = [
  // frame-ancestors in the CSP is the modern form; X-Frame-Options is kept for
  // the proxies and scanners that still only read this one.
  { key: "X-Frame-Options", value: "DENY" },
  { key: "X-Content-Type-Options", value: "nosniff" },
  { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
  // Nothing in the console asks for any of these.
  { key: "Permissions-Policy", value: "camera=(), microphone=(), geolocation=(), payment=()" },
];

/**
 * Dev-only: Next blocks cross-origin requests to /_next dev assets, which a tunnel
 * always is. Reaching the console through one is how Slack's OAuth redirect and the
 * live-browser share links get exercised locally, so the tunnel hosts are allowed by
 * wildcard, free tunnel hostnames rotate, and pinning one means editing this file
 * every restart. Ignored entirely by `next build`.
 */
const DEV_TUNNEL_ORIGINS = ["*.ngrok-free.app", "*.ngrok.app", "*.trycloudflare.com"];

const nextConfig: NextConfig = {
  output: "standalone",
  devIndicators: false,
  allowedDevOrigins: DEV_TUNNEL_ORIGINS,
  // This is a running App Router application. Express owns /api and the
  // browser WebSockets; all frontend requests go to the Next.js runtime.
  outputFileTracingRoot: __dirname,
  turbopack: { root: __dirname },
  async headers() {
    return [{ source: "/:path*", headers: SECURITY_HEADERS }];
  },
};

export default nextConfig;
