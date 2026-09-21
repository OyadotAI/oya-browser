/**
 * Looking at the page: a screenshot, and the list of visible elements.
 */
import { useState } from 'react';
import { api, errorMessage } from '@/lib/api-client';
import { withBusy, type PanelContext } from './context';
import type { InputResult, PageElement } from './types';

/** The data a screenshot command answers with. */
interface ScreenshotData {
  /** The image as a data URL. */
  screenshot?: string;
}

/** The data an analyze command answers with. */
interface AnalyzeData {
  /** Every element found on the page. */
  elements?: PageElement[];
}

/**
 * Looking needs no control lease: while the agent drives, the agent path
 * serves it without pausing anyone. Human input is refused until taken.
 */
function capture(ctx: PanelContext, controlMode: string): Promise<InputResult> {
  if (controlMode === 'human') return ctx.send('screenshot');
  const body = { action: 'screenshot' };
  return api<InputResult>(`/browsers/${ctx.browserId}/command`, { key: ctx.apiKey, method: 'POST', body }).catch(
    (e) => (ctx.toast(errorMessage(e), 'error'), { ok: false }),
  );
}

/** Takes a screenshot and shows it, if one came back. */
async function screenshot(ctx: PanelContext, controlMode: string, setShot: (s: string) => void) {
  const r = await withBusy(ctx.setBusy, 'screenshot', () => capture(ctx, controlMode));
  const data = (r.data as ScreenshotData | undefined)?.screenshot;
  if (data) setShot(data);
}

/** Lists the page's visible elements, if the analyze succeeded. */
async function analyze(ctx: PanelContext, setElements: (e: PageElement[]) => void) {
  const r = await withBusy(ctx.setBusy, 'analyze', () => ctx.send('analyze'));
  const els = (r.data as AnalyzeData | undefined)?.elements;
  if (els) setElements(els.filter((e) => e.visible));
}

/** `shot` and `elements` are shown until dismissed; `screenshot` and `analyze` fetch them. */
export function useInspect(ctx: PanelContext, controlMode: string) {
  const [shot, setShot] = useState<string | null>(null);
  const [elements, setElements] = useState<PageElement[] | null>(null);
  return {
    ...{ shot, setShot, elements, setElements },
    screenshot: () => screenshot(ctx, controlMode, setShot),
    analyze: () => analyze(ctx, setElements),
  };
}
