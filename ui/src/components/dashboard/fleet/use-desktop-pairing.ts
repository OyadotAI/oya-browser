/**
 * The desktop browser signs in as this key's identity, and cloud browsers inherit
 * those logins. The link carries a single-use pairing code, never the key itself.
 */
import { useState } from 'react';
import { desktopSignInUrl } from '../config';
import { useToast } from '../toast';

/** Mints a pairing link and hands it to the installed desktop browser, toasting the outcome. */
async function openDesktop(apiKey: string, toast: ReturnType<typeof useToast>) {
  try {
    window.location.href = await desktopSignInUrl(apiKey);
    toast('Opening the desktop browser. Not installed yet? Download it from /downloads.', 'info');
  } catch (err) {
    toast(err instanceof Error ? err.message : 'Could not start desktop sign-in', 'error');
  }
}

/** `connectDesktop` opens the paired desktop browser; `pairing` is true while the code is minted. */
export function useDesktopPairing(apiKey: string) {
  const toast = useToast();
  const [pairing, setPairing] = useState(false);
  const connectDesktop = async () => {
    setPairing(true);
    await openDesktop(apiKey, toast).finally(() => setPairing(false));
  };
  return { pairing, connectDesktop };
}
