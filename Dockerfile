# ── Stage 1: Build Next.js UI ──
FROM node:24-alpine AS ui-build

WORKDIR /ui
COPY ui/package.json ui/package-lock.json ./
RUN npm ci
COPY ui/ ./
RUN npm run build

# ── Stage 2: Production server ──
FROM node:24-alpine

WORKDIR /app

# Managed runtime operators may mount a Docker socket explicitly.
RUN apk add --no-cache docker-cli

# Install server deps (production only)
COPY server/package.json server/package-lock.json ./
RUN npm ci --omit=dev

# Server source
COPY server/src/ ./src/

# The page analyzer and the fingerprint injection, both of which drivers/cdp.js
# loads relative to its own file (../../../browser/...), so the layout matters:
# without these, CDP browsers lose analyze, click-by-element-id, and every
# fingerprint patch — silently.
COPY browser/scripts/ /browser/scripts/
COPY browser/anonymity/ /browser/anonymity/
COPY browser/login-state.js /browser/login-state.js

# Standard Next.js runtime, traced dependencies, and static assets
COPY --from=ui-build /ui/.next/standalone/ /ui/.next/standalone/

# Browser binaries for download (populated by CI before build)
COPY server/downloads/ ./downloads/

EXPOSE 3100

ENV NODE_ENV=production
ENV PORT=3100
ENV OYA_UI_MODE=production

CMD ["node", "src/index.ts"]
