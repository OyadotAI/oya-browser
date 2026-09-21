/**
 * Everything the browser panel runs on, composed from the smaller hooks:
 * the polled detail, live frames, input, navigation, inspection and sharing.
 */
import { useState } from 'react';
import { useToast } from '../toast';
import type { PanelContext } from './context';
import type { Busy } from './types';
import { useBrowserDetail } from './use-browser-detail';
import { useSend } from './use-send';
import { useLiveFrames } from './use-live-frames';
import { useNavigation } from './use-navigation';
import { useInspect } from './use-inspect';
import { useCopyId } from './use-copy-id';

/** What the panel's state depends on. */
export interface PanelInputs {
  /** The project key. */
  apiKey: string;
  /** The browser shown. */
  browserId: string;
  /** Closes the panel; called when the browser is gone. */
  onClose: () => void;
}

/** The detail, the busy slot, and the context the action hooks share. */
export function usePanelContext({ apiKey, browserId, onClose }: PanelInputs) {
  const toast = useToast();
  const [busy, setBusy] = useState<Busy>(null);
  const d = useBrowserDetail(browserId, apiKey, onClose);
  const send = useSend(browserId, apiKey, toast);
  const { detail, onInput, refresh } = d;
  const ctx: PanelContext = { browserId, apiKey, detail, send, onInput, refresh, setBusy, toast };
  return { ...d, busy, ctx };
}

/** The whole panel's state: detail, context, live frames, URL bar, inspection and copy-id. */
export function useBrowserPanel(inputs: PanelInputs) {
  const core = usePanelContext(inputs);
  const live = useLiveFrames(inputs.browserId, inputs.apiKey);
  const nav = useNavigation(core.ctx);
  const inspect = useInspect(core.ctx, core.controlMode);
  return { ...core, live, nav, inspect, copy: useCopyId(inputs.browserId) };
}

/** What `useBrowserPanel` returns. */
export type PanelState = ReturnType<typeof useBrowserPanel>;
