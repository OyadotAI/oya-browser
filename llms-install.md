# Installing Oya Browser for an AI agent

Oya is the browser an agent works in. Two ways to connect, no code to build.

## On a computer with a person (preferred)

1. The person installs the desktop app from https://oyabrowser.com/#download
   (macOS .dmg, Windows .exe). On first run it imports their logins from their
   usual browser, on the same computer; nothing is sent anywhere.
2. The person clicks **Connect** next to your name in the Oya window footer
   (Claude Code, Codex, Cursor and VS Code appear when installed). That writes
   an MCP server named `oya-browser` at `http://127.0.0.1:47321/mcp` into your
   config, with the token in the OS keychain.
3. Start a new session. Use the `oya-browser` tools for every web task. Each
   session drives its own tab; the person can take control at any time.

## Without a desktop (a server, a container, CI)

Connect to the cloud endpoint with an Oya API key from https://oyabrowser.com:

    claude mcp add --transport http oya https://oyabrowser.com/mcp/pool \
      --header "Authorization: Bearer $OYA_API_KEY"

Any MCP client works the same way with the URL and the header. `initialize`
and `tools/list` answer without a key, so you can read the tool list first.
Call `start_browser` once, then the page tools.

## The full reference

Setup, every tool, the SDK and the REST API: https://oyabrowser.com/llms.txt
