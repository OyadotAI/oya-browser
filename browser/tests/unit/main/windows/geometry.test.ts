/** Native drag feedback stays inside the pointer's display. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { previewPosition } from '../../../../src/main/windows/geometry.ts';
it('keeps a preview on a negative-coordinate monitor at its bottom-right corner', () => {
  const point = previewPosition({ x: -10, y: 890 }, { x: -1600, y: 0, width: 1600, height: 900 });
  assert.ok(point.x >= -1600 && point.x + 300 <= 0);
  assert.ok(point.y >= 0 && point.y + 228 <= 900);
});
