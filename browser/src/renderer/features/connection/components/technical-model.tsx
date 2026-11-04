/** Equal provider choices ask for one model key, then report a real test independently from saving. */
import { useEffect, useRef } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Button } from '../../../ui/index.ts';
import type { TechnicalModelViewModel, TechnicalProvider } from '../view-models/technical-model-view-model.ts';
import { FIRST_TASK } from '../model/setup-constants.ts';

/** Names distinguish an API credential from the separate coding-agent subscription path. */
const PROVIDERS: {
  /** Stable server ID. */ id: TechnicalProvider;
  /** Visible provider name. */ label: string;
  /** Small decorative mark. */ mark: string;
}[] = [
  { id: 'gemini', label: 'Gemini', mark: 'G' },
  { id: 'anthropic', label: 'Claude API', mark: 'C' },
  { id: 'openrouter', label: 'OpenRouter', mark: 'OR' },
];
/** Choosing any provider has identical visual weight and no default selection. */
function ProviderChoices({ vm }: { /** Draft provider state. */ vm: TechnicalModelViewModel }) {
  const s = useViewModel(vm);
  return (
    <div className="technical-providers" role="group" aria-label="Model provider">
      {PROVIDERS.map((provider) => (
        <button
          type="button"
          key={provider.id}
          aria-pressed={s.provider === provider.id}
          disabled={s.busy}
          onClick={() => vm.choose(provider.id)}
        >
          <span className="technical-provider-mark" aria-hidden="true">
            {provider.mark}
          </span>
          <strong>{provider.label}</strong>
        </button>
      ))}
    </div>
  );
}
/** An explicit selection or refused credential brings keyboard focus to its masked input. */
function useKeyFocus(provider: string, code: string) {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (provider) input.current?.focus();
  }, [provider, code]);
  return input;
}
/** The model stays on its server default unless the person opens advanced settings. */
function ModelField({ vm }: { /** Catalog-derived default and optional override. */ vm: TechnicalModelViewModel }) {
  const s = useViewModel(vm);
  return (
    <details className="setup-help">
      <summary>Advanced model</summary>
      <label className="technical-model-field">
        Model ID
        <input
          value={s.model}
          disabled={s.busy}
          placeholder="Use the provider default"
          onChange={(event) => vm.setModel(event.target.value)}
        />
      </label>
    </details>
  );
}
/** Key input is never cached in storage, returned by main or displayed as plain text. */
function Credentials({ vm }: { /** Selected provider and key live only in this form. */ vm: TechnicalModelViewModel }) {
  const s = useViewModel(vm),
    provider = PROVIDERS.find((item) => item.id === s.provider);
  const input = useKeyFocus(s.provider, s.code);
  if (!provider) return null;
  return (
    <form
      className="technical-model-form"
      onSubmit={(event) => {
        event.preventDefault();
        void vm.connect();
      }}
    >
      <label className="technical-model-field" htmlFor="technical-model-key">
        {provider.id === 'anthropic' ? 'Claude API key' : `${provider.label} API key`}
        <input
          ref={input}
          id="technical-model-key"
          type="password"
          autoComplete="off"
          spellCheck={false}
          value={s.key}
          disabled={s.busy || s.ready}
          onChange={(event) => vm.edit(event.target.value)}
          aria-describedby="technical-key-help"
        />
      </label>
      <p className="setup-detail" id="technical-key-help">
        {s.provider === 'anthropic'
          ? 'Use an API key, not a Claude Code setup token.'
          : 'Your key is saved to your Oya workspace.'}
      </p>
      <p className="setup-detail">{s.model ? `Model: ${s.model}` : 'Uses the provider’s default model.'}</p>
      <ModelField vm={vm} />
      {!s.ready && (
        <>
          <Button type="submit" variant="primary" disabled={s.busy}>
            {s.busy ? 'Connecting…' : s.saved ? 'Retry test' : 'Connect and test'}
          </Button>
          <p className="setup-detail">Sends one small test request. Sign in with Oya if needed.</p>
        </>
      )}
    </form>
  );
}
/** A successful provider response is visibly distinct from a saved key or opened login page. */
export function TechnicalModel({
  vm,
}: {
  /** Existing configuration APIs and bounded diagnostics. */ vm: TechnicalModelViewModel;
}) {
  const s = useViewModel(vm),
    provider = PROVIDERS.find((item) => item.id === s.provider);
  return (
    <>
      <h2 tabIndex={-1}>Choose your model provider.</h2>
      <ProviderChoices vm={vm} />
      <Credentials vm={vm} />
      {s.progress && <p role="status">{s.progress}</p>}
      {s.error && (
        <p className="setup-error" role="alert">
          {s.error}
        </p>
      )}
      {s.ready && (
        <section className="setup-first-task">
          <strong role="status">{provider?.label} ready.</strong>
          <p>Try this in Oya’s Ask panel:</p>
          <p>{FIRST_TASK}</p>
          <Button variant="primary" onClick={() => vm.finish()}>
            Open browser
          </Button>
          <Button onClick={() => vm.edit('')}>Edit key</Button>
        </section>
      )}
    </>
  );
}
