/**
 * Copies the browser id and flashes a checkmark.
 */
import { useState } from 'react';
import { COPIED_FLASH_MS } from './constants';

/** `copied` is true for a moment after `copyId` succeeds. */
export function useCopyId(browserId: string) {
  const [copied, setCopied] = useState(false);
  const flash = () => {
    setCopied(true);
    setTimeout(() => setCopied(false), COPIED_FLASH_MS);
  };
  return { copied, copyId: () => void navigator.clipboard.writeText(browserId).then(flash) };
}
