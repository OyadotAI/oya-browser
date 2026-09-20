/**
 * Unit tests for the Control tab's pure formatting and classification rules.
 */
import { describe, it, expect } from 'vitest';
import {
  auditTarget,
  barTone,
  bytes,
  duration,
  errorRateTone,
  frameGap,
  healthFigures,
  limitIsLow,
  metricLabel,
  num,
  outcomeClass,
  providerBody,
  providerStatus,
  usageCell,
  usageRows,
  usageWarns,
  usedPct,
} from '@/components/dashboard/control/format';
import type { AuditEvent, Provider, Usage } from '@/components/dashboard/control/types';

/** A provider with every counter at rest. */
const provider = (over: Partial<Provider> = {}): Provider => ({
  name: 'p',
  type: 'cdp',
  active: 0,
  maxConcurrent: 1,
  priority: 1,
  weight: 1,
  healthy: true,
  available: true,
  latencyMs: null,
  cooldownMsRemaining: 0,
  totalSessions: 0,
  totalFailures: 0,
  ...over,
});

describe('control format', () => {
  it('turns camelCase and snake_case metric names into a sentence', () => {
    expect(metricLabel('commandsPerMinute')).toBe('Commands per minute');
    expect(metricLabel('chat_tokens')).toBe('Chat tokens');
  });

  it('shows a dash for a missing number', () => {
    expect(num(undefined)).toBe('—');
    expect(num(null)).toBe('—');
  });

  it('picks the largest byte unit that keeps the value above one', () => {
    expect(bytes(0)).toBe('0 B');
    expect(bytes(512)).toBe('512 B');
    expect(bytes(1536)).toBe('1.5 KB');
  });

  it('formats durations as seconds, minutes or hours', () => {
    expect(duration(42)).toBe('42s');
    expect(duration(185)).toBe('3m 5s');
    expect(duration(7800)).toBe('2h 10m');
  });

  it('computes the error rate and throttling from usage, zero without commands', () => {
    expect(healthFigures(undefined)).toEqual({ commands: 0, errors: 0, errorRate: 0, throttled: 0 });
    const u = { commands: 50, command_errors: 5, rate_limited: 2, quota_denied: 1 } as unknown as Usage;
    expect(healthFigures(u)).toEqual({ commands: 50, errors: 5, errorRate: 10, throttled: 3 });
  });

  it('colours the error rate green, yellow past 2% and red past 10%', () => {
    expect(errorRateTone(2)).toBe('good');
    expect(errorRateTone(5)).toBe('warn');
    expect(errorRateTone(11)).toBe('bad');
  });

  it('caps capacity use at 100% and colours it by how full it is', () => {
    expect(usedPct(3, 0)).toBe(0);
    expect(usedPct(5, 2)).toBe(100);
    expect(barTone(50)).toBe('bg-accent');
    expect(barTone(80)).toBe('bg-amber-500');
    expect(barTone(95)).toBe('bg-red-500');
  });

  it('flags a live limit under a fifth of its burst, never a disabled one', () => {
    expect(limitIsLow({ limit: 10, burst: 10, remaining: 1 })).toBe(true);
    expect(limitIsLow({ limit: 10, burst: 10, remaining: 5 })).toBe(false);
    expect(limitIsLow({ limit: 10, burst: 10, remaining: 0, disabled: true })).toBe(false);
  });

  it('describes a provider as unconnected, ready or cooling down', () => {
    expect(providerStatus(provider())).toBe('Not yet connected');
    expect(providerStatus(provider({ totalSessions: 1 }))).toBe('Ready for connections');
    expect(providerStatus(provider({ totalFailures: 1, healthy: false }))).toBe(
      'Connection failed · retrying after cooldown',
    );
  });

  it('sends a CDP provider its URL and no key, in the server’s field order', () => {
    const body = providerBody({
      name: ' a ',
      type: 'cdp',
      wsUrl: ' ws://x ',
      apiKey: 'k',
      maxConcurrent: '5',
      priority: '100',
      weight: '1',
    });
    expect(JSON.stringify(body)).toBe(
      '{"name":"a","type":"cdp","wsUrl":"ws://x","maxConcurrent":5,"priority":100,"weight":1}',
    );
  });

  it('sends a vendor its trimmed key only when one was typed', () => {
    const draft = {
      name: 'b',
      type: 'steel',
      wsUrl: 'ws://x',
      apiKey: ' k ',
      maxConcurrent: '1',
      priority: '0',
      weight: '2',
    };
    expect(JSON.stringify(providerBody(draft))).toBe(
      '{"name":"b","apiKey":"k","type":"steel","maxConcurrent":1,"priority":0,"weight":2}',
    );
    expect(providerBody({ ...draft, apiKey: '  ' })).not.toHaveProperty('apiKey');
  });

  it('shortens audit targets and colours outcomes', () => {
    const e = { target_type: 'browser', target_id: 'abcdefghijklmnop' } as AuditEvent;
    expect(auditTarget(e, 4)).toBe('browser abcd');
    expect(auditTarget({ ...e, target_type: null }, 4)).toBe('—');
    expect(outcomeClass('ok')).toBe('text-accent');
    expect(outcomeClass('denied')).toBe('text-yellow');
    expect(outcomeClass('error')).toBe('text-red');
  });

  it('clamps the pause between frames and falls back when timing is missing', () => {
    expect(
      frameGap(
        [
          { i: 0, t: 0 },
          { i: 1, t: 10 },
        ],
        0,
      ),
    ).toBe(80);
    expect(
      frameGap(
        [
          { i: 0, t: 0 },
          { i: 1, t: 5000 },
        ],
        0,
      ),
    ).toBe(1000);
    expect(
      frameGap(
        [
          { i: 0, t: 0 },
          { i: 1, t: 0 },
        ],
        0,
      ),
    ).toBe(200);
  });

  it('formats browser time and bytes out, and warns on non-zero throttling', () => {
    const u = { hour: 'h', openBrowsers: 2, browser_seconds: 65, bytes_out: 2048, rate_limited: 1 } as unknown as Usage;
    const rows = usageRows(u);
    expect(rows[1]).toEqual(['Browsers open now', 2]);
    expect(usageCell('Browser time', null, u)).toBe('1m 5s');
    expect(usageCell('Bytes out', null, u)).toBe('2.0 KB');
    expect(usageWarns('Rate limited', 1)).toBe(true);
    expect(usageWarns('Rate limited', 0)).toBe(false);
    expect(usageWarns('Commands', 9)).toBe(false);
  });
});
