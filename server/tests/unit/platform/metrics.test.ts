/**
 * Unit tests for in-process metrics: counters, gauges and histograms, their
 * Prometheus exposition, the JSON snapshot, and the per-metric series cap.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { metrics, render, reset, snapshot } from '../../../src/platform/metrics.ts';
import { HISTOGRAM_BUCKETS, MAX_SERIES_PER_METRIC } from '../../../src/platform/constants.ts';

describe('metrics', () => {
  beforeEach(() => reset());

  it('adds to a counter per label set', () => {
    metrics.commands.inc({ action: 'click', outcome: 'ok' });
    metrics.commands.inc({ outcome: 'ok', action: 'click' }, 2);
    metrics.commands.inc({ action: 'click', outcome: 'error' });
    const series = snapshot().oya_commands_total;
    assert.equal(series.length, 2, 'label order does not make a new series');
    assert.equal(series.find((s) => s.labels.outcome === 'ok').value, 3);
  });

  it('renders HELP, TYPE and one line per series, with labels sorted', () => {
    metrics.commands.inc({ outcome: 'ok', action: 'click' });
    const text = render();
    assert.match(
      text,
      /# HELP oya_commands_total Browser commands by action and outcome\n# TYPE oya_commands_total counter\n/,
    );
    assert.match(text, /oya_commands_total\{action="click",outcome="ok"\} 1\n/);
  });

  it('escapes quotes, backslashes and newlines in label values', () => {
    metrics.commands.inc({ action: 'a"b\\c\nd' });
    assert.ok(render().includes('oya_commands_total{action="a\\"b\\\\c\\nd"} 1'));
  });

  it('leaves out a metric with no series', () => {
    assert.ok(!render().includes('oya_commands_total'));
  });

  it('sets, raises and lowers a gauge', () => {
    metrics.browsersConnected.set({}, 5);
    metrics.browsersConnected.inc();
    metrics.browsersConnected.dec({}, 3);
    assert.equal(snapshot().oya_browsers_connected[0].value, 3);
  });

  it('samples a process gauge when read', () => {
    assert.ok(snapshot().oya_process_uptime_seconds[0].value > 0);
    assert.match(render(), /oya_process_rss_bytes \d+/);
  });

  it('renders histogram buckets cumulatively, with sum and count', () => {
    metrics.commandDuration.observe({}, 3);
    metrics.commandDuration.observe({}, 7);
    metrics.commandDuration.observe({}, 1e9);
    const text = render();
    assert.match(text, /oya_command_duration_ms_bucket\{le="5"\} 1\n/);
    assert.match(text, /oya_command_duration_ms_bucket\{le="10"\} 2\n/);
    assert.match(text, /oya_command_duration_ms_bucket\{le="\+Inf"\} 3\n/);
    assert.match(text, /oya_command_duration_ms_count 3\n/);
  });

  it('ignores a histogram value that is not a finite number', () => {
    metrics.commandDuration.observe({}, NaN);
    assert.deepEqual(snapshot().oya_command_duration_ms, []);
  });

  it('snapshots a histogram with its average and bucket percentiles', () => {
    for (let i = 0; i < 99; i++) metrics.commandDuration.observe({}, 4);
    metrics.commandDuration.observe({}, 900);
    const [s] = snapshot().oya_command_duration_ms;
    assert.equal(s.count, 100);
    assert.equal(s.avg, (99 * 4 + 900) / 100);
    assert.equal(s.p50, HISTOGRAM_BUCKETS[0]);
    assert.equal(s.p99, HISTOGRAM_BUCKETS[0]);
  });

  it('has no percentile for a value beyond the last bucket', () => {
    metrics.commandDuration.observe({}, 1e9);
    assert.equal(snapshot().oya_command_duration_ms[0].p50, null);
  });

  it('drops new series once a metric holds the maximum', () => {
    for (let i = 0; i <= MAX_SERIES_PER_METRIC; i++) metrics.frames.inc({ client: `c${i}` });
    metrics.frames.inc({ client: 'c0' });
    const series = snapshot().oya_frames_total;
    assert.equal(series.length, MAX_SERIES_PER_METRIC);
    assert.equal(series[0].value, 2, 'an existing series still counts');
  });
});
