# ── Oya Browser, Docker (headless Electron via Xvfb) ──
FROM node:24-bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive

# Install Xvfb, dbus, and all Electron/Chromium dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    xvfb \
    x11-utils \
    dbus \
    dbus-x11 \
    # Chromium/Electron shared libraries
    libgtk-3-0 \
    libnotify4 \
    libnss3 \
    libxss1 \
    libxtst6 \
    libgbm1 \
    libasound2 \
    libatk1.0-0 \
    libatk-bridge2.0-0 \
    libcups2 \
    libdrm2 \
    libxcomposite1 \
    libxdamage1 \
    libxrandr2 \
    libpango-1.0-0 \
    libcairo2 \
    libxkbcommon0 \
    libatspi2.0-0 \
    # Fonts
    fonts-liberation \
    fonts-noto-color-emoji \
    # VNC (optional, for debugging)
    x11vnc \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Install dependencies
COPY package.json package-lock.json* ./
RUN npm ci

# Copy browser source
COPY main.js preload.js login-state.js governance.js cdp-front-door.js shell-layout.cjs control-state.cjs constants.cjs ./
COPY main/ ./main/
COPY front-door/ ./front-door/
COPY renderer/ ./renderer/
COPY scripts/ ./scripts/
COPY anonymity/ ./anonymity/
COPY build/ ./build/

# Copy entrypoint
COPY docker-entrypoint.sh /docker-entrypoint.sh
RUN chmod +x /docker-entrypoint.sh

# Display
ENV DISPLAY=:99

# Screen resolution
ENV SCREEN_WIDTH=1920
ENV SCREEN_HEIGHT=1080
ENV SCREEN_DEPTH=24

# Docker mode
ENV OYA_DOCKER=true
LABEL ai.getoya.governance="1"

# VNC off by default
ENV VNC_ENABLED=false
ENV VNC_PORT=5900

EXPOSE 5900

# Not root. This is the one container that renders attacker-chosen web content,
# and uid 0 plus --no-sandbox means a renderer bug is a root shell in the
# container. --no-sandbox stays in the entrypoint: dropping it also needs
# unprivileged user namespaces (or --cap-add SYS_ADMIN), which is a decision
# about the host's runtime, not something an image can make.
# Xvfb creates its socket directory only as root; without this it falls back to
# an abstract socket and logs an error on every boot.
RUN mkdir -p /tmp/.X11-unix && chmod 1777 /tmp/.X11-unix
RUN useradd --create-home --uid 10001 oya && chown -R oya:oya /app
USER oya
ENV HOME=/home/oya

ENTRYPOINT ["/docker-entrypoint.sh"]
