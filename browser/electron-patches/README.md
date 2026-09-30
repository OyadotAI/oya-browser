# Electron patches

Oya ships a patched Electron. Each patch here applies to the Electron release in
`../package.json` (now **v44.4.3**, Chromium 152.0.7977.130).

## 0001-trust-anchor-ids.patch

Every Chrome since 147 sends the TLS `trust_anchors` extension (0xca34) in its
ClientHello. Chrome fills the list in `chrome/browser/ssl/ssl_config_service_manager.cc`
from the compiled-in Chrome Root Store; Electron has no such layer, so Oya's ClientHello
lacked the extension and its JA4 was one no Chrome has:

| Browser | JA4 |
|:--|:--|
| Google Chrome 154 | `t13d1517h2_8daaf6152771_…` |
| Oya on stock Electron 44 (and Electron 45 alpha) | `t13d1516h2_8daaf6152771_806a8c22fdea` |

Anti-bot vendors (DataDome, Akamai, Cloudflare) read JA4 before any page script runs.
Flags cannot fix it: `--enable-features=TLSTrustAnchorIDs` has nothing to send when the
list is empty. The patch sets `SSLConfig.trust_anchor_ids` for every session, as Chrome
152's `GetNewSSLConfig` does. It changes no driver, window or automation behaviour.

The list is the one compiled into this Chromium; Electron has no component updater, so
it refreshes when Oya moves to a newer Electron.

### Build

A first Electron build needs ~100 GB of disk and a large machine (32+ cores recommended).

```sh
npm i -g @electron/build-tools
e init oya-44 --root=~/electron --import=release   # checks out Electron + Chromium
cd ~/electron/src/electron && git checkout v44.4.3 && e sync
git apply /path/to/agentchrome/browser/electron-patches/0001-trust-anchor-ids.patch
e build                                             # then: e build electron:dist
```

Build for every platform Oya ships: macOS arm64/x64 (desktop), Linux x64/arm64 (the
container image), Windows x64.

### Verify

Point the app at `https://tls.peet.ws/api/all`: `tls.ja4` must read `t13d1517h2_…` and
`tls.extensions` must include `51764`. `bu-benchmark`'s `wire.py` does this for a
running Oya; the benchmark's `oya_candidate` row is the end-to-end check.
