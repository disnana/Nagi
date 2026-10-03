import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { runInNewContext } from 'node:vm';

const source = readFileSync(new URL('../compiler/src/graph/html.rs', import.meta.url), 'utf8');
const helper = source.split('\n').find(line => line.trimStart().startsWith('function viewportBounds('));
assert.ok(helper, 'The HTML viewer must include the geometry used for its viewBox.');
const boundsFunction = runInNewContext(helper + '\nviewportBounds;');
const viewport = (...args) => JSON.parse(JSON.stringify(boundsFunction(...args)));

test('cross-module call curves fit with marker and stroke space on both sides', () => {
  // Actual SVG bounds from supervised-service: number_response -> empty/json.
  const drawing = { x: -25.405517578125, y: 28, width: 938.8110961914062, height: 3056 };
  const box = viewport(890, 3108, drawing);
  assert.deepEqual(box, { x: -38, y: 0, width: 964, height: 3108 });
  assert.ok(box.x <= drawing.x - 12);
  assert.ok(box.x + box.width >= drawing.x + drawing.width + 12);
});

test('ordinary stock-report layout keeps its existing extent', () => {
  assert.deepEqual(viewport(890, 420, { x: 22, y: 28, width: 844, height: 368 }),
    { x: 0, y: 0, width: 890, height: 420 });
});

test('geometry above or below the layout extends the vertical viewport', () => {
  const drawing = { x: 22, y: -8, width: 844, height: 444 };
  assert.deepEqual(viewport(890, 420, drawing), { x: 0, y: -20, width: 890, height: 468 });
});

test('an empty SVG bounding box does not add blank padding', () => {
  assert.deepEqual(viewport(700, 380, { x: 0, y: 0, width: 0, height: 0 }),
    { x: 0, y: 0, width: 700, height: 380 });
});

test('non-finite browser measurements preserve the finite layout extent', () => {
  for (const key of ['x', 'y', 'width', 'height']) {
    for (const value of [NaN, Infinity, -Infinity]) {
      assert.deepEqual(viewport(700, 380, { x: 22, y: 28, width: 200, height: 300, [key]: value }),
        { x: 0, y: 0, width: 700, height: 380 });
    }
  }
});
