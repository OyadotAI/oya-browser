/**
 * Which panel each settings section shows: a map from section to renderer.
 */
'use client';

import type { ReactNode } from 'react';
import type { KeyConfig } from '../config';
import SlackSection from '../slack-section';
import WebhookSection from '../webhook-section';
import BrowsersSection from './browsers-section';
import type { Section } from './constants';
import { Row } from './fields';
import ModelSection from './model-section';
import type { SettingsEditorState } from './use-settings-editor';
import VerificationSection from './verification-section';

/** What a panel renderer gets. */
interface PanelProps {
  /** The editor state. */
  editor: SettingsEditorState;
  /** The loaded settings. */
  config: KeyConfig;
  /** The key being configured; Slack and webhooks save themselves with it. */
  apiKey: string;
}

/** Section → panel. Slack and webhooks save on their own, so they only need the key. */
export const PANELS: Record<Section, (props: PanelProps) => ReactNode> = {
  model: ({ editor, config }) => <ModelSection config={config} form={editor.form} choice={editor.choice} />,
  browsers: ({ editor, config }) => (
    <BrowsersSection config={config} form={editor.form} pairing={editor.pairing} openDesktop={editor.openDesktop} />
  ),
  verification: ({ editor, config }) => <VerificationSection config={config} form={editor.form} />,
  alerts: ({ apiKey }) => <SlackSection apiKey={apiKey} Row={Row} />,
  webhooks: ({ apiKey }) => <WebhookSection apiKey={apiKey} Row={Row} />,
};
