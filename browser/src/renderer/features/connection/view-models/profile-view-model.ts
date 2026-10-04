/**
 * "Browsing as" and "This device" on the account page: which profile this
 * browser browses as (its cookies and logins, in plain words), the device
 * sites see it as, and its name, saved as it is changed.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { ConnectionStatus } from '../../../../shared/ipc.ts';
import { PLATFORM_NAMES, TEXT } from '../model/constants.ts';
import { messageOf, shapeOrNull, type Config, type Device } from '../model/models.ts';

/** What the profile groups know. */
export interface ProfileState {
  /** The persona's name ('' or 'Default' for Oya's own). */
  profileName: string;
  /** The persona's device, or null before one is assigned. */
  device: Device | null;
  /** The name field as typed. */
  name: string;
  /** The name last saved. */
  savedName: string;
  /** What the last rename said ('' for nothing). */
  nameStatus: string;
}

/** The parts of the bridge the profile groups use. */
export type ProfileBridge = Pick<OyaBrowser, 'getFingerprint' | 'onFingerprintChanged' | 'saveConfig' | 'onWsStatus'>;

/** The persona in words. */
export interface PersonaText {
  /** Its name, quoted, or "Oya's own profile". */
  name: string;
  /** What it means for logins. */
  detail: string;
}

/** The persona in words: its name, and what it means for logins. */
export function personaText(profileName: string): PersonaText {
  const name = profileName && profileName !== 'Default' ? profileName : '';
  if (!name)
    return {
      name: 'Oya’s own profile',
      detail: 'Logins you make here are kept in this profile. Import to bring your own browser’s.',
    };
  return { name: `“${name}”`, detail: 'Oya signs in to sites with this profile’s cookies and logins.' };
}

/** The device in words ("Windows · Europe/London · en-GB · 1920x1080"), or that there is none yet. */
export function deviceText(device: Device | null): string {
  if (!device) return 'This computer, until you connect';
  const platform = device.platform ?? '';
  const named = Object.hasOwn(PLATFORM_NAMES, platform) ? PLATFORM_NAMES[platform] : platform;
  return [named, device.timezone, device.locale, device.screen].filter(Boolean).join(' · ');
}

/** The profile groups. */
export class ProfileViewModel extends ViewModel<ProfileState> {
  /** The main process. */
  private readonly bridge: ProfileBridge;

  /** Reads the device, following persona changes and the connection's profile name. */
  constructor(bridge: ProfileBridge) {
    super({ profileName: '', device: null, name: '', savedName: '', nameStatus: '' });
    this.bridge = bridge;
    this.own(bridge.onFingerprintChanged((device) => this.set({ device: shapeOrNull<Device>(device) })));
    this.own(bridge.onWsStatus((status) => this.onStatus(status)));
    bridge.getFingerprint().then(
      (device) => this.set({ device: shapeOrNull<Device>(device) }),
      () => this.set({ device: null }),
    );
  }

  /** The saved name. */
  loadConfig(config: Config): void {
    const name = config.browserName ?? '';
    this.set({ name, savedName: name });
  }

  /** Names the profile from the connection's status. */
  onStatus(status: ConnectionStatus): void {
    this.set({ profileName: status.profileName ?? '' });
  }

  /** The name field was typed in. */
  type(name: string): void {
    this.set({ name, nameStatus: '' });
  }

  /** The name was changed (the field lost focus): save it; an empty or unchanged one is put back. */
  async rename(): Promise<void> {
    const [name, saved] = [this.state.name.trim(), this.state.savedName];
    if (!name || name === saved) return this.set({ name: saved });
    this.set({ savedName: name });
    const said = await this.bridge.saveConfig({ browserName: name }).then(
      () => TEXT.nameSaved,
      (e: unknown) => messageOf(e),
    );
    this.set({ nameStatus: said });
  }
}
