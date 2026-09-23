#!/bin/bash
set -e

# ── Start D-Bus ──
if [ -z "$DBUS_SESSION_BUS_ADDRESS" ]; then
  eval $(dbus-launch --sh-syntax)
  export DBUS_SESSION_BUS_ADDRESS
fi

# ── Start Xvfb ──
rm -f /tmp/.X99-lock
Xvfb :99 -screen 0 "${SCREEN_WIDTH:-1920}x${SCREEN_HEIGHT:-1080}x${SCREEN_DEPTH:-24}" \
  -ac -nolisten tcp +extension GLX &
XVFB_PID=$!

# Wait for Xvfb to be ready
for i in $(seq 1 10); do
  if xdpyinfo -display :99 >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done

# ── Optional VNC ──
if [ "$VNC_ENABLED" = "true" ]; then
  x11vnc -display :99 -forever -shared -rfbport "${VNC_PORT:-5900}" \
    -nopw -xkb -noxrecord -noxfixes -noxdamage &
  echo "[oya-docker] VNC server started on port ${VNC_PORT:-5900}"
fi

# ── Graceful shutdown ──
cleanup() {
  echo "[oya-docker] Shutting down..."
  if [ -n "${ELECTRON_PID:-}" ]; then
    kill "$ELECTRON_PID" 2>/dev/null || true
    wait "$ELECTRON_PID" 2>/dev/null || true
  fi
  kill $XVFB_PID 2>/dev/null || true
  exit 0
}
trap cleanup SIGTERM SIGINT

# ── Start Electron ──
# --no-sandbox must be a CLI arg, Electron checks for root before app code runs
#
# --disable-dev-shm-usage is not optional here. /dev/shm is 64MB in a container,
# and every live-stream frame goes through viz CopyOutputResult, which wants
# shared memory for a full 1920x1080 capture. The allocation fails, the mojo
# message is rejected, the GPU process dies, and after six deaths Chromium
# gives up with "GPU process isn't usable. Goodbye." and SIGTRAPs the whole app.
# Streaming a browser for ~20s was enough to kill it every time.
echo "[oya-docker] Starting Oya Browser (${SCREEN_WIDTH:-1920}x${SCREEN_HEIGHT:-1080})"
./node_modules/.bin/electron . --no-sandbox --disable-gpu --disable-dev-shm-usage &
ELECTRON_PID=$!

# ── Hard lifetime ──
# Docker, Kubernetes and ECS have no idle stop of their own, so the server sets
# OYA_MAX_LIFETIME_MINUTES and the browser stops itself then, whatever the runtime.
if [ -n "${OYA_MAX_LIFETIME_MINUTES:-}" ]; then
  (sleep "$((OYA_MAX_LIFETIME_MINUTES * 60))" && echo "[oya-docker] Lifetime of ${OYA_MAX_LIFETIME_MINUTES}m reached" && kill -TERM $ELECTRON_PID) &
fi

wait $ELECTRON_PID
cleanup
