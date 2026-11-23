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
 * The desktop install scripts are read as text by `curl | sh` and PowerShell's `irm`. Left to its
 * extension, the .ps1 would go out as a download, and `irm` would hand back bytes, not a script.
 */
const INSTALL_SCRIPT_HEADERS = [{ key: "Content-Type", value: "text/plain; charset=utf-8" }];

/**
 * Dev-only: Next blocks cross-origin requests to /_next dev assets, which a tunnel
 * always is. Reaching the console through one is how Slack's OAuth redirect and the
 * live-browser share links get exercised locally, so the tunnel hosts are allowed by
 * wildcard, free tunnel hostnames rotate, and pinning one means editing this file
 * every restart. Ignored entirely by `next build`.
 */
const DEV_TUNNEL_ORIGINS = ["*.ngrok-free.app", "*.ngrok.app", "*.trycloudflare.com", "host.docker.internal"];

/**
 * Short addresses buyers and assistants guess for the healthcare and comparison pages. Each one
 * points permanently at the page that already ranks for it, so the guess lands and no second page
 * competes with the first for the same search.
 */
const ALIASES: [string, string][] = [
  ["/security", "/trust"],
  ["/hipaa", "/baa"],
  ["/healthcare", "/browser-for-healthcare"],
  ["/use-cases/payer-portal-automation", "/payer-portal-automation"],
  ["/use-cases/eligibility-verification", "/eligibility-verification-automation"],
  ["/use-cases/prior-authorization", "/prior-authorization-automation"],
  ["/use-cases/claim-status", "/claim-status-automation"],
  ["/compare/:vendor", "/vs/:vendor"],
];

const nextConfig: NextConfig = {
  async redirects() {
    return ALIASES.map(([source, destination]) => ({ source, destination, permanent: true }));
  },
  output: "standalone",
  devIndicators: false,
  allowedDevOrigins: DEV_TUNNEL_ORIGINS,
  // This is a running App Router application. Express owns /api and the
  // browser WebSockets; all frontend requests go to the Next.js runtime.
  outputFileTracingRoot: __dirname,
  turbopack: { root: __dirname },
  async headers() {
    return [
      { source: "/:path*", headers: SECURITY_HEADERS },
      { source: "/install-desktop.:shell(sh|ps1)", headers: INSTALL_SCRIPT_HEADERS },
    ];
  },
};

export default nextConfig;
