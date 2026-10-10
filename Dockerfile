# ── Stage 1: Build Next.js UI ──
FROM node:24-alpine AS ui-build

WORKDIR /ui
COPY ui/package.json ui/package-lock.json ./
RUN npm ci
COPY ui/ ./
# The release notes page is built from the changelog at the repository root.
COPY CHANGELOG.md /CHANGELOG.md
# The trust page is built from the generated HIPAA evidence pack.
COPY compliance/evidence.json /compliance/evidence.json
RUN npm run build

# ── Stage 2: Production server ──
FROM node:24-alpine

WORKDIR /app

# Managed runtime operators may mount a Docker socket explicitly.
RUN apk add --no-cache docker-cli

# Install server deps (production only)
COPY server/package.json server/package-lock.json ./
RUN npm ci --omit=dev

# Server source, and its vendor/ folder beside it
COPY server/src/ ./src/
COPY server/vendor/ ./vendor/

# What the server shares with the browser, loaded relative to its own files
# (../../../browser/...), so the layout matters: the page analyzer
# (scripts/analyzer.js) and the fingerprint injection (anonymity/), whose loss
# costs CDP browsers analyze, click-by-element-id and every fingerprint patch
# silently; and the TypeScript the server imports: the workflow model, the page
# helpers and the persona applier (src/package.json makes them ES modules).
COPY browser/scripts/ /browser/scripts/
COPY browser/anonymity/ /browser/anonymity/
COPY browser/src/package.json /browser/src/package.json
COPY browser/src/workflow/ /browser/src/workflow/
COPY browser/src/page/ /browser/src/page/
COPY browser/src/anonymity/ /browser/src/anonymity/
# The release version /health reports (src/platform/version.ts), bumped with the SDK by the release script.
COPY browser/package.json /browser/package.json

# Standard Next.js runtime, traced dependencies, and static assets
COPY --from=ui-build /ui/.next/standalone/ /ui/.next/standalone/

# Browser binaries for download (populated by CI before build)
COPY server/downloads/ ./downloads/

EXPOSE 3100

ENV NODE_ENV=production
ENV PORT=3100
ENV OYA_UI_MODE=production

CMD ["node", "src/index.ts"]
