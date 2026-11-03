/** Measured zero, tiny positive values and unavailable metrics must remain distinguishable. */
import { describe, expect, it } from 'vitest';
import { coverageText, measurementNote, runtimeText } from '@/components/admin/keel-model';
import { KEEL_SNAPSHOT } from './keel-fixture';

describe('Keel measurement display', () => {
  it('does not replace missing runtime with measured zero', () => {
    expect(runtimeText(null)).toBe('—');
    expect(runtimeText(0)).toBe('0.0 h');
    expect(runtimeText(1000)).toBe('<0.1 h');
    expect(runtimeText(3_600_000)).toBe('1.0 h');
  });

  it('states partial and unavailable measurement coverage', () => {
    expect(coverageText(0, 10)).toBe('Not reported');
    expect(coverageText(2, 3)).toBe('Partial coverage · 2 of 3 daily reports');
    expect(coverageText(3, 3)).toBe('3 of 3 daily reports measured');
    expect(measurementNote(KEEL_SNAPSHOT.usage)).toContain('measured reports only');
    expect(measurementNote({ ...KEEL_SNAPSHOT.usage, runtime_reports: 0, project_reports: 0 })).toContain(
      'Existing reports do not include',
    );
  });
});
