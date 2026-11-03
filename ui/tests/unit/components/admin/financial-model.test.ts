/** Staff money displays preserve API currency units and distinguish unavailable amounts from measured zero. */
import { expect, it } from 'vitest';
import { financialText, moneyText } from '@/components/admin/financial-model';

it('formats verified API units without adding different currencies', () => {
  expect(moneyText({ currency: 'jpy', amountMinor: 1200, minorUnitExponent: 0 })).toBe('JPY 1,200');
  expect(moneyText({ currency: 'ugx', amountMinor: 120000, minorUnitExponent: 2 })).toBe('UGX 1,200.00');
  expect(
    financialText({
      state: 'complete',
      values: [
        { currency: 'usd', amountMinor: 101, minorUnitExponent: 2 },
        { currency: 'jpy', amountMinor: 5, minorUnitExponent: 0 },
      ],
      reason: '',
    }),
  ).toBe('USD 1.01 · JPY 5');
});

it('renders incomplete or absent data as coverage states, never as zero', () => {
  expect(financialText(null)).toBe('—');
  expect(financialText({ state: 'incomplete', values: null, reason: 'limit' })).toBe('Incomplete');
  expect(financialText({ state: 'complete', values: [], reason: '' })).toBe('0');
});
