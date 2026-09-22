/**
 * The `oya help` text: every command and flag, grouped, the exit codes, and
 * where the saved config lives. A test holds it to the command maps and to
 * the flags the parser knows, so neither can grow without the other.
 */
import { configPath } from './config.ts';

/** What `oya help` prints. */
export const HELP = `oya, thousands of browsers, one API

Usage: oya <command> [arguments] [options]

Set up
  oya install [--dry-run] [--config <file>]  Stand up a self-hosted control plane
  oya login [--key <key>] [--url <url>]      Save an API key for this machine
  oya init                                   Set your model, browser provider and sign-ins
  oya whoami                                 Which control plane and key are in use, and from where

Browsers
  oya start                                  Start a browser and print its id
      [--persona <id|auto>] [--name <name>] [--idempotency-key <key>]
      [--provider cdp --ws-url ws://127.0.0.1:9222]        on a Chrome you run
      [--governed] [--queue-ms 30000] [--budget-usd 5] [--priority low|normal|high] [--policy '<json>']
  oya goto <url> [--id <id>]                 Navigate (defaults to the newest browser)
  oya ask "<prompt>" [--id <id>]             Drive it in plain language
  oya ls | list                              List running browsers
  oya status [--id <id>]                     Health, counters and what it has been doing
  oya open [--id <id>]                       Open the live view in your browser
  oya rm | stop <id>... | --all              Stop browsers

Personas and cookies
  oya personas                               List identities: fingerprint + cookies + proxy
  oya personas new | create [name]           [--name <n>] [--platform Win32|MacIntel|Linux] [--tz <zone>]
                                             [--locale <l>] [--max <n>] [--geo <cc>] [--preview]
  oya personas edit <id>                     [--name <n>] [--max <n|none>] [--geo <cc>]
  oya personas clone <id> [--name <n>]       A new device of the same kind
  oya personas rm | delete <id>...           Delete personas
  oya cookies export <persona>               Logins to stdout; [--out <file>] [--format json|playwright]
  oya cookies import <persona> <file>        Logins from a JSON file
  oya cookies copy <from> <to>               One persona's logins into another
  oya cookies clear <persona>                Delete a persona's logins

Account
  oya config [key=value ...]                 Show or change this key's settings
  oya usage                                  What this key has spent
  oya stealth-test [--live]                  Score this deployment against bot detectors

Control plane (always prints JSON)
  oya control                                Durable project overview
  oya sessions [<id>]                        All sessions, including pending cleanup
  oya cancel <id>                            Stop a session in any state; prints its final state
  oya stop <id> --force                      Stop despite a profile-save error, or reconcile
  oya recover <id> [--replace [--ws-url ws://host:9222]]
                                             Recover in place, or replace it (a cdp session needs --ws-url)
  oya takeover <id>                          Take human control
  oya release <id>                           Hand control back, leaving the agent paused
  oya resume <id>                            Let the agent carry on
  oya events [--after <cursor>]              Read durable lifecycle events
  oya project '<settings-json>'              Update limits, rate cards and retention
  oya members                                List the owner and members
  oya members invite [--role <role>]         An invitation code; role viewer|operator|administrator
  oya members remove <user>                  Remove a member
  oya credential new [--role <role>] [--label <text>]   Mint a service credential (shown once)
  oya credential revoke <id>                 Revoke a service credential
  oya webhook new <https-url>                Register a signed event webhook
  oya webhook remove <id>                    Disable a webhook
  oya webhook replay <id>                    Send a delivery again

Options
  --json          One JSON document on stdout; errors as JSON on stderr
  --url <url>     Control plane for this call (else OYA_BASE_URL, else the one oya login saved)
  --key <key>     API key for this call (else OYA_API_KEY, else the one oya login saved)
  --debug         On failure, also print the status, the response body and the stack
  --version       Print the version
  --help, -h      This text; the command is not run

Exit codes: 0 done · 1 failed, or no answer · 2 usage error, nothing was sent
Config: ${configPath}
`;
