/**
 * The "Connect an AI model" card (`#chat-model`): the provider chips, the
 * model picker, the key, and Save. It takes focus on the key when it opens.
 */
import { useEffect, useRef } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Button, Icon } from '../../../ui/index.ts';
import './model-card.css';
import { ASK_TEXT } from '../model/constants.ts';
import { keyHint, type ModelSetupViewModel } from '../view-models/model-setup-view-model.ts';
import { ModelPicker } from './model-picker.tsx';

/** What the card's parts are given. */
interface ModelCardProps {
  /** The setup cards. */
  model: ModelSetupViewModel;
}

/** The provider chips: one radio per provider in the server's catalog. */
function ProviderChips({ model }: ModelCardProps) {
  const { status, picked } = useViewModel(model);
  return (
    <div
      className="provider-chips"
      id="chat-model-provider"
      role="radiogroup"
      aria-labelledby="chat-model-provider-label"
    >
      {status.catalog.map((p) => (
        <button
          key={p.id}
          type="button"
          className="provider-chip"
          role="radio"
          data-provider={p.id}
          aria-checked={p.id === picked}
          onClick={() => model.choose(p.id)}
        >
          <span className="provider-mark">{p.label.slice(0, 1)}</span>
          <span className="provider-name">{p.label}</span>
        </button>
      ))}
    </div>
  );
}

/** The key field, with what it expects; focused when the card opens. */
function KeyField({ model }: ModelCardProps) {
  const { open, key, status } = useViewModel(model);
  const field = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (open) field.current?.focus();
  }, [open]);
  const hint = keyHint(status, model.entry);
  return (
    <div className="model-field">
      <label className="model-label" htmlFor="chat-model-key">
        API key
      </label>
      <input
        type="password"
        id="chat-model-key"
        ref={field}
        autoComplete="off"
        spellCheck={false}
        aria-describedby="chat-model-key-help"
        placeholder={hint.placeholder}
        value={key}
        onChange={(e) => model.setKey(e.target.value)}
      />
      <span className="model-help" id="chat-model-key-help">
        {hint.help}
      </span>
    </div>
  );
}

/** The card's head: "Connect an AI model" until the project has a key, then "AI model". */
function ModelCardHead({ model }: ModelCardProps) {
  const { status } = useViewModel(model);
  return (
    <div className="model-card-head">
      <span className="tool-mark" data-icon="spark">
        <Icon name="spark" />
      </span>
      <div>
        <h2 id="chat-model-title">{status.hasLlmKey ? ASK_TEXT.modelTitle : ASK_TEXT.connectTitle}</h2>
        <p>Ask runs on your own key. It is saved to your project, so the console shows the same.</p>
      </div>
    </div>
  );
}

/** Advanced endpoint configuration shares the project's existing server setting. */
function EndpointField({ model }: ModelCardProps) {
  const { baseUrl } = useViewModel(model);
  return (
    <details>
      <summary>Advanced endpoint</summary>
      <label className="model-field">
        API base URL
        <input
          type="url"
          value={baseUrl}
          placeholder="Provider default"
          onChange={(e) => model.setEndpoint(e.target.value)}
        />
        <span className="model-help">Leave empty to use the provider default.</span>
      </label>
    </details>
  );
}

/** The model card. */
export function ModelCard({ model }: ModelCardProps) {
  const { open, optional, error } = useViewModel(model);
  return (
    <form
      className="chat-card model-card"
      id="chat-model"
      hidden={!open}
      onSubmit={(e) => (e.preventDefault(), void model.save())}
    >
      <ModelCardHead model={model} />
      <div className="model-field">
        <span className="model-label" id="chat-model-provider-label">
          Provider
        </span>
        <ProviderChips model={model} />
      </div>
      <div className="model-field">
        <span className="model-label" id="chat-model-model-label">
          Model
        </span>
        <ModelPicker picker={model.picker} />
      </div>
      <KeyField model={model} />
      <EndpointField model={model} />
      <p className="chat-card-error" id="chat-model-error" role="alert" hidden={!error}>
        {error}
      </p>
      <div className="chat-card-actions">
        <Button type="button" id="chat-model-get" onClick={() => model.getKey()}>
          Get a key
        </Button>
        <Button type="button" id="chat-model-cancel" hidden={!optional} onClick={() => model.hide()}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" id="chat-model-save">
          Save
        </Button>
      </div>
    </form>
  );
}
