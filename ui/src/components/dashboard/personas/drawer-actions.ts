/**
 * What the profile drawer's buttons do, one function each. Every action runs
 * through the drawer's `act`, so it marks itself busy, toasts a failure and
 * reloads the tab on success.
 */
import { desktopSignInUrl } from '../config';
import { newMfa } from './mfa';
import * as personas from './persona-api';
import type { DrawerCtx } from './use-persona-drawer';

/** Saves the name, cap and geo hint. */
export const save = ({ d, p, props }: DrawerCtx) =>
  d.act('save', () => personas.updatePersona(props.apiKey, p, d.fields), 'Saved');

/** Pins the persona to the chosen proxy, or back to auto. */
export const savePin = ({ d, p, props }: DrawerCtx) =>
  d.act('pin', () => personas.pinProxy(props.apiKey, p.id, d.fields.pin), d.fields.pin ? 'Pinned' : 'Unpinned');

/** Makes a new identity on the same kind of device. */
export const clone = ({ d, p, props }: DrawerCtx) =>
  d.act('clone', async () => {
    const c = await personas.clonePersona(props.apiKey, p.id);
    d.toast(`Created ${c.name}, same kind of device, new identity`, 'success');
  });

/** Deletes the persona and closes the drawer. */
export const remove = ({ d, p, props }: DrawerCtx) =>
  d.act(
    'delete',
    async () => {
      await personas.deletePersona(props.apiKey, p.id);
      d.setConfirmDelete(false);
      props.onClose();
    },
    'Deleted',
  );

/** Opens the desktop browser signed in as this persona. */
export const pair = ({ d, p, props }: DrawerCtx) =>
  d.act('pair', async () => {
    window.location.href = await desktopSignInUrl(props.apiKey, p.id);
  });

/** Stores the drafted factor, then empties the draft (kept on its type) whether or not it was stored. */
export const storeFactor = ({ d, p, props }: DrawerCtx) =>
  d
    .act('mfa', () => personas.storeMfa(props.apiKey, p.id, d.mfa), 'Second factor stored')
    .then(() => d.setMfa(newMfa(d.mfa.type)));

/** Removes the default factor, or the one for `domain`. */
export const clearFactor = ({ d, p, props }: DrawerCtx, domain?: string) =>
  d.act('mfa', () => personas.clearMfa(props.apiKey, p.id, domain), 'Second factor removed');

/** Stores the drafted sign-in, then empties the draft whether or not it was stored. */
export const addCredential = ({ d, p, props }: DrawerCtx) =>
  d
    .act('cred', () => personas.storeCredential(props.apiKey, p.id, d.cred), 'Sign-in stored')
    .then(() => d.setCred({ domain: '', username: '', password: '' }));

/** Removes the sign-in for `domain`. */
export const removeCredential = ({ d, p, props }: DrawerCtx, domain: string) =>
  d.act('cred', () => personas.removeCredential(props.apiKey, p.id, domain), 'Sign-in removed');
