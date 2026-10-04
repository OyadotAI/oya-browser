/**
 * The format an analysis's page is written in, decided in one place: the
 * caller's (a server command's `format`), else the one chosen in settings
 * (`ui.pageFormat`), else markdown. The renderers are src/page/render.ts.
 */
import { withPage, type AnalyzeResult } from '../../page/render.ts';
import type { CommandParams, ScriptResult } from './types.ts';

/** The display settings that name a page format. */
interface UiSettings {
  /** The format chosen in settings. */
  pageFormat?: string;
}

/** The saved settings, as far as the page format goes. */
interface SavedSettings {
  /** Display settings. */
  ui?: UiSettings;
}

/** The settings store. */
interface SettingsStore {
  /** The saved settings. */
  readonly values?: SavedSettings;
}

/** Where the saved page format is read from: the app's settings. */
export interface FormatSettings {
  /** The settings store, whose `values.ui.pageFormat` names the format chosen. */
  config?: SettingsStore;
}

/** An analyze result with its page rendered in the format that applies. */
export function renderedAnalysis(
  source: FormatSettings,
  result: unknown,
  params?: Pick<CommandParams, 'format'>,
): ScriptResult | null {
  const analysis = result && typeof result === 'object' ? (result as AnalyzeResult) : null;
  return withPage(analysis, params?.format || source.config?.values?.ui?.pageFormat);
}
