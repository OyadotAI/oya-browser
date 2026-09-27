/**
 * The license form's state: what is typed, the key just issued (shown once),
 * and why issuing failed.
 */
import { useState } from 'react';
import { DEFAULT_LICENSE_BROWSERS, DEFAULT_LICENSE_DAYS } from './constants';
import { daysFromNow } from './model';
import type { AdminState } from './use-admin';

/** The form's fields. */
interface Fields {
  /** Who the license is for. */
  licensee: string;
  /** Cloud browsers at once, as typed. */
  browsers: string;
  /** Last day, YYYY-MM-DD. */
  expires: string;
}

/** The fields, starting from a year's license for the default number of browsers. */
function useFields() {
  const [fields, setFields] = useState<Fields>(() => ({
    licensee: '',
    browsers: String(DEFAULT_LICENSE_BROWSERS),
    expires: daysFromNow(DEFAULT_LICENSE_DAYS),
  }));
  return { fields, set: (patch: Partial<Fields>) => setFields((f) => ({ ...f, ...patch })) };
}

/** The license the fields ask for. */
const requestOf = (f: Fields) => ({ licensee: f.licensee, maxConcurrent: Number(f.browsers), expiresAt: f.expires });

/** Everything the license form shows and does. */
export function useLicenseForm(issue: AdminState['issue']) {
  const { fields, set } = useFields();
  const [outcome, setOutcome] = useState({ key: '', error: '' });
  const submit = () =>
    issue(requestOf(fields)).then(
      (issued) => setOutcome({ key: issued.key, error: '' }),
      (e) => setOutcome({ key: '', error: e instanceof Error ? e.message : 'Could not issue the license' }),
    );
  return { fields, set, submit, ...outcome };
}
