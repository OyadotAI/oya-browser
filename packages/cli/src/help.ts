/**
 * The `oya help` text: every command and flag, and where the saved config lives.
 */
import { configPath } from './config.ts';

/** What `oya help` prints. */
export const HELP = `oya, thousands of browsers, one API

  oya install                     Stand up a self-hosted control plane
            [--dry-run] [--config oya-install.json]
  oya login                       Save an API key for this machine
  oya init                        Set your model, browser provider and sign-ins
  oya start [--persona auto]      Start a browser and print its id
  oya goto <url> [--id <id>]      Navigate (defaults to the newest browser)
  oya ask "<prompt>" [--id <id>]  Drive it in plain language
  oya ls                          List running browsers
  oya rm <id> | --all             Stop browsers
  oya personas                    Identities: fingerprint + cookies + proxy
  oya personas new [name]         --platform Win32|MacIntel|Linux --tz <zone> --locale <l> --max <n>
  oya personas edit <id>          --name <n> --max <n> --geo <cc>
  oya personas clone|rm <id>      A new device of the same kind · delete
  oya cookies export <persona>    Logins to stdout or --out <file>; --format playwright for addCookies()
  oya cookies import <persona> <file>   Logins from a JSON file
  oya cookies copy <from> <to>    One persona's logins into another
  oya status [--id <id>]          Health, counters and what it has been doing
  oya open [--id <id>]            Open the live view in your browser
  oya config [key=value ...]      Show or change this key's settings
  oya control                     Durable project overview
  oya sessions [id]               All sessions, including pending cleanup
  oya stop <id> --force           Stop despite a profile-save error, or reconcile
  oya takeover <id>               Acquire human control
  oya release <id>                Release human control, leaving the agent paused
  oya resume <id>                 Acknowledge agent resume
  oya events [--after <cursor>]   Read durable lifecycle events
  oya project <settings-json>     Update limits, rate cards and retention
  oya start --governed --provider oya-selfhosted [--queue-ms 30000]
            [--budget-usd 5] [--policy JSON] [--idempotency-key ID]
  oya cancel <id>               Cancel queued or provisioning work
  oya recover <id> [--replace]   Explicitly recover or replace a session
  oya members [invite|remove]   List members, invite, or remove a user
  oya credential new [--role viewer|operator|administrator]
  oya credential revoke <id>     Revoke a service credential
  oya webhook new <https-url>    Register a signed event webhook
  oya webhook remove <id>        Disable a webhook
  oya webhook replay <id>        Replay a delivery
  oya usage                       What this key has spent
  oya stealth-test [--live]       Score this deployment against bot detectors

Options: --url <control plane>   --key <api key>   --json
Config:  ${configPath}
`;
