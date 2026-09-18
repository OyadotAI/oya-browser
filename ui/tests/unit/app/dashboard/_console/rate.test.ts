/**
 * Unit tests for the fleet throughput between two polls.
 */
import { describe, it, expect } from 'vitest';
import { commandRate } from '@/app/dashboard/_console/rate';

describe('commandRate', () => {
  it('needs an earlier sample', () => {
    expect(commandRate(null, { at: 1, commands: 1, errors: 0 })).toBeNull();
    expect(commandRate({ at: 5, commands: 0, errors: 0 }, { at: 5, commands: 9, errors: 0 })).toBeNull();
  });

  it('gives commands per minute and the error share', () => {
    const r = commandRate({ at: 0, commands: 100, errors: 10 }, { at: 30_000, commands: 160, errors: 25 });
    expect(r).toEqual({ commandsPerMin: 120, errorPct: 25 });
  });

  it('counts counters that went backwards (a restart) as zero', () => {
    expect(commandRate({ at: 0, commands: 50, errors: 5 }, { at: 60_000, commands: 3, errors: 1 })).toEqual({
      commandsPerMin: 0,
      errorPct: 0,
    });
  });
});
