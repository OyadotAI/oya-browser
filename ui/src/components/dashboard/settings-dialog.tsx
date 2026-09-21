/**
 * The settings dialog: the key's AI model, browser provider, verification,
 * alerts and webhooks. The parts live in settings/.
 */
'use client';

import SettingsEditor, { type SettingsProps } from './settings/settings-editor';

/**
 * A new editing session starts with saved values. Cancel never leaves a
 * hidden draft behind, and switching API keys cannot carry credentials over.
 */
export default function SettingsDialog(props: SettingsProps) {
  return props.open ? <SettingsEditor key={props.apiKey} {...props} /> : null;
}
