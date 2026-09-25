/**
 * The "AI model" panel: provider, model, API key, and an advanced base URL.
 */
'use client';

import { Check } from 'lucide-react';
import { llmCatalog, type KeyConfig } from '../config';
import { CUSTOM_MODEL, SAVED_PLACEHOLDER } from './constants';
import { Row, Secret, Select } from './fields';
import { SectionHeading } from './heading';
import type { SettingsForm } from './use-draft';
import type { ModelChoice } from './use-model-choice';

/** What the model panel edits. */
interface Props {
  /** The saved settings. */
  config: KeyConfig;
  /** The draft. */
  form: SettingsForm;
  /** Provider and model state. */
  choice: ModelChoice;
}

/** One button per provider the server offers; the chosen one is ticked. */
function ProviderPicker({ config, choice }: Omit<Props, 'form'>) {
  return (
    <div role="group" aria-label="AI provider" className="grid grid-cols-3 gap-2">
      {llmCatalog(config).map((p) => {
        const on = choice.provider === p.id;
        const tone = on
          ? 'border-accent/50 bg-accent/[0.06] text-text'
          : 'border-border bg-bg-sunken text-text-muted hover:border-text-dim';
        return (
          <button
            key={p.id}
            type="button"
            aria-pressed={on}
            onClick={() => choice.chooseProvider(p.id)}
            className={`flex h-11 items-center justify-between rounded-lg border px-3.5 text-[13px] font-medium ${tone}`}
          >
            <span>{p.label}</span>
            {on && <Check className="h-3.5 w-3.5 text-accent" />}
          </button>
        );
      })}
    </div>
  );
}

/** The model select, with a free-text id when "Custom model…" is chosen. */
function ModelPicker({ form, choice }: Omit<Props, 'config'>) {
  return (
    <Row id="settings-model" label="Model" hint="Use a preset or your own model ID.">
      <Select id="settings-model" value={choice.custom ? CUSTOM_MODEL : choice.model} onChange={choice.pickModel}>
        {choice.models.map((m) => (
          <option key={m.id} value={m.id}>
            {m.label}
          </option>
        ))}
        <option value={CUSTOM_MODEL}>Custom model…</option>
      </Select>
      {choice.custom && (
        <input
          aria-label="Custom model ID"
          className="settings-input mt-2 font-mono text-[12px]"
          value={choice.model}
          placeholder="Enter a model ID"
          onChange={(e) => form.set('chat_model', e.target.value)}
        />
      )}
    </Row>
  );
}

/** The API key for the chosen provider. A switched provider needs a new one. */
function ModelKey({ config, form, choice }: Props) {
  const preset = llmCatalog(config).find((p) => p.id === choice.provider);
  const kept = !choice.providerChanged && config.openai_api_key;
  return (
    <Row
      id="settings-model-key"
      label="API key"
      hint={
        config.inherited && !choice.providerChanged
          ? 'Using the server’s shared key.'
          : 'Your credential stays private.'
      }
    >
      <Secret
        id="settings-model-key"
        value={form.draft.openai_api_key ?? ''}
        onChange={(v) => form.set('openai_api_key', v)}
        placeholder={kept ? SAVED_PLACEHOLDER : preset?.hint || 'Enter your API key'}
      />
      {choice.needsModelKey && (
        <p className="mt-2 text-[12px] leading-5 text-yellow">Enter a key for {preset?.label} to switch providers.</p>
      )}
    </Row>
  );
}

/** A compatible gateway's base URL, folded away. */
function AdvancedConnection({ form }: { /** The draft. */ form: SettingsForm }) {
  return (
    <details className="mt-6 border-t border-border pt-4">
      <summary className="cursor-pointer text-[12px] font-medium text-text-muted hover:text-text">
        Advanced connection
      </summary>
      <div className="mt-4">
        <Row id="settings-base-url" label="API base URL" hint="For a compatible gateway.">
          <input
            id="settings-base-url"
            className="settings-input font-mono text-[12px]"
            type="url"
            value={form.value('openai_base_url')}
            onChange={(e) => form.set('openai_base_url', e.target.value)}
            placeholder="Use provider default"
          />
          <p className="mt-2 text-[11.5px] leading-5 text-text-muted">A custom endpoint requires your own API key.</p>
        </Row>
      </div>
    </details>
  );
}

/** The panel. */
export default function ModelSection(props: Props) {
  return (
    <>
      <SectionHeading eyebrow="Intelligence" title="Choose how Oya thinks.">
        The model behind your browser conversations.
      </SectionHeading>
      <div className="space-y-5">
        <Row label="Provider" hint="Connect your AI account.">
          <ProviderPicker config={props.config} choice={props.choice} />
        </Row>
        <ModelPicker form={props.form} choice={props.choice} />
        <ModelKey {...props} />
      </div>
      <AdvancedConnection form={props.form} />
    </>
  );
}
