/**
 * The "Verification" panel: the CAPTCHA solver and where MFA lives.
 */
'use client';

import { KeyRound } from 'lucide-react';
import type { KeyConfig } from '../config';
import { SAVED_PLACEHOLDER } from './constants';
import { Row, Secret, Select } from './fields';
import { SectionHeading } from './heading';
import type { SettingsForm } from './use-draft';

/** The panel. A solver key is asked for only once a solver is chosen. */
export default function VerificationSection({
  config,
  form,
}: {
  /** The saved settings. */
  config: KeyConfig;
  /** The draft. */
  form: SettingsForm;
}) {
  return (
    <>
      <SectionHeading eyebrow="Continuity" title="Keep the session moving.">
        Choose how browsers handle verification prompts.
      </SectionHeading>
      <div className="space-y-5">
        <Row id="settings-captcha" label="CAPTCHA solver" hint="Optional automatic solving.">
          <Select
            id="settings-captcha"
            value={form.value('captcha_solver')}
            onChange={(v) => form.set('captcha_solver', v)}
          >
            <option value="">Manual · ask for help</option>
            <option value="capsolver">CapSolver</option>
          </Select>
        </Row>
        {form.value('captcha_solver') && (
          <Row id="settings-captcha-key" label="Solver API key" hint="From your solver account.">
            <Secret
              id="settings-captcha-key"
              value={form.draft.captcha_api_key ?? ''}
              onChange={(v) => form.set('captcha_api_key', v)}
              placeholder={config.captcha_api_key ? SAVED_PLACEHOLDER : 'Enter solver API key'}
            />
          </Row>
        )}
      </div>
      <div className="mt-7 flex gap-3 border-t border-border pt-5">
        <KeyRound className="mt-0.5 h-4 w-4 shrink-0 text-text-muted" />
        <div>
          <h4 className="text-[13px] font-medium">Two-factor authentication</h4>
          <p className="mt-1 text-[12px] leading-5 text-text-muted">
            Manage MFA inside each profile. If a prompt needs your attention, finish it in the live browser.
          </p>
        </div>
      </div>
    </>
  );
}
