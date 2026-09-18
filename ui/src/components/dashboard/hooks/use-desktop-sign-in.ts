/**
 * Opens the installed desktop browser signed in with this key, through a
 * single-use pairing link (see `desktopSignInUrl`). A failure is toasted.
 */
import { desktopSignInUrl } from '../config';
import { useBusyAction } from './use-busy-action';

/** `open(profile?)` hands the desktop a pairing link; `busy` is true while the link is being made. */
export function useDesktopSignIn(apiKey: string) {
  const { busy, run } = useBusyAction();
  const open = (profile?: string) =>
    run(async () => {
      window.location.href = await desktopSignInUrl(apiKey, profile);
    });
  return { busy, open };
}
