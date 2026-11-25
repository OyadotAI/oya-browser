/** Private static terminal scripts read executable arguments as data, with no interpolated commands or credentials. */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import type { AgentCommand } from './agent-platform.ts';
import { AGENT_LAUNCH_MODE } from './constants.ts';
import { PRIVATE_FILE_MODE } from '../app/constants.ts';

/** Static POSIX wrapper quotes every argument and never evaluates generated shell source. */
const MAC_RUNNER =
  '#!/bin/sh\nIFS= read -r executable < "$0.executable"\nset --\nwhile IFS= read -r argument; do set -- "$@" "$argument"; done < "$0.arguments"\nprintf started > "$0.started"\nexec "$executable" "$@"\n';
/** Constant PowerShell reads JSON as data; no execution-policy setting is changed. */
const WINDOWS_DOCUMENT =
  "@echo off\r\nset \"OYA_AGENT_LAUNCH_DIR=%~dp0\"\r\npowershell.exe -NoProfile -NoExit -Command \"$ErrorActionPreference='Stop'; $root=$env:OYA_AGENT_LAUNCH_DIR; $m=Get-Content -Raw -LiteralPath (Join-Path $root 'manifest.json') | ConvertFrom-Json; $a=@($m.args); Set-Content -LiteralPath (Join-Path $root 'open-agent.cmd.started') -Value 'started'; & $m.file @a\"\r\n";
/** A launch lease contains no token, prompt or user-supplied command. */
export interface LaunchLease {
  /** Native document opened by the OS. */ file: string;
  /** Checks only the bounded wrapper-start marker. */ started(): boolean;
  /** Removes only this handoff's private temporary directory. */ close(): void;
}
/** Filesystem operations stay in this repository rather than the launch service. */
export class LaunchRepository {
  /** Create private native documents for a previously detected executable. */
  prepare(command: AgentCommand, platform: string): LaunchLease {
    validateCommand(command);
    const dir = privateDirectory();
    try {
      return this.create(dir, command, platform);
    } catch (error) {
      fs.rmSync(dir, { recursive: true, force: true });
      throw error;
    }
  }
  /** Each script is constant source; only private data files contain detected argument values. */
  private create(dir: string, command: AgentCommand, platform: string): LaunchLease {
    const file = path.join(dir, platform === 'win32' ? 'open-agent.cmd' : 'open-agent.command');
    if (platform === 'win32') this.windows(dir, command, file);
    else this.mac(command, file);
    return {
      file,
      started: () => fs.existsSync(`${file}.started`),
      close: () => fs.rmSync(dir, { recursive: true, force: true }),
    };
  }
  /** Argument boundaries survive spaces, quotes, punctuation and JavaScript entrypoints. */
  private mac(command: AgentCommand, file: string): void {
    fs.writeFileSync(file, MAC_RUNNER, { mode: AGENT_LAUNCH_MODE });
    fs.writeFileSync(`${file}.executable`, `${command.file}\n`, { mode: PRIVATE_FILE_MODE });
    fs.writeFileSync(`${file}.arguments`, command.args.map((arg) => `${arg}\n`).join(''), { mode: PRIVATE_FILE_MODE });
  }
  /** Windows uses a static native document and a JSON manifest, never command-string concatenation. */
  private windows(dir: string, command: AgentCommand, file: string): void {
    fs.writeFileSync(file, WINDOWS_DOCUMENT, { mode: PRIVATE_FILE_MODE });
    fs.writeFileSync(path.join(dir, 'manifest.json'), JSON.stringify(command), { mode: PRIVATE_FILE_MODE });
  }
}

/** A private parent prevents another OS user replacing argument manifests or scripts. */
function privateDirectory(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-agent-launch-'));
  fs.chmodSync(dir, AGENT_LAUNCH_MODE);
  return dir;
}

/** Newlines and NUL cannot be represented by the private POSIX argument manifest. */
function validateCommand(command: AgentCommand): void {
  if ([command.file, ...command.args].some((value) => /[\r\n\0]/.test(value)))
    throw Error('Open this agent manually, then paste the test prompt.');
}
