/**
 * Copies the SDK example with the real key filled in, and remembers that it
 * did so the button can say so.
 */
import { useState } from 'react';
import { useToast } from '../toast';
import type { ShowToast } from '../toast/types';
import { sdkCode } from './sdk-code';

/** `copy()` writes the example to the clipboard; `copied` turns true once it has. */
export function useCopyCode(apiKey: string, profileId: string) {
  const toast = useToast();
  const [copied, setCopied] = useState(false);
  const copy = () => writeCode(sdkCode(apiKey, profileId), () => setCopied(true), toast);
  return { copied, copy };
}

/** Copies `code`, then calls `done`; a refused clipboard gets an error toast instead. */
async function writeCode(code: string, done: () => void, toast: ShowToast) {
  try {
    await navigator.clipboard.writeText(code);
    done();
  } catch {
    toast('Could not copy. Select the code and copy it manually.', 'error');
  }
}
