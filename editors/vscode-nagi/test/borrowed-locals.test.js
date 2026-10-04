'use strict';
const assert = require('node:assert/strict');
const test = require('node:test');
const features = require('../src/features');

for (const [suffix, low] of [['nagi', false], ['low', true]]) {
  test(`${suffix} borrowed loop locals keep their declared type and bare insertion`, () => {
    const file = `/project/main.${suffix}`;
    const text = 'print("😀"); print(item)\r\nprint(item)';
    const start = text.indexOf('item');
    const location = { file, line: 1, column: start + 1, length: 4 };
    const local = { name: 'item', type: 'Entry', borrowed: true, readonly: true, location };
    const index = { definitions: [{ name: 'item', kind: 'function', signature: 'def item() -> i64', parameters: [] }], locals: [local] };
    const source = { file };
    const hover = features.hoverAt(index, text, start + 2, source).item;
    assert.equal(hover.signature, 'item: Entry (read-only borrow)');
    assert.equal(hover.type, 'Entry');
    assert.equal(hover.kind, 'variable');
    assert.equal(hover.borrowed, true);
    assert.equal(hover.readonly, true);
    const items = features.completionCandidates(index, text, start + 2, low, source).filter(item => item.name === 'item');
    assert.equal(items.length, 1, 'a borrowed local shadows a callable of the same name');
    assert.equal(items[0].signature, hover.signature);
    assert.equal(items[0].type, 'Entry');
    assert.equal(items[0].kind, 'variable');
    assert.equal(features.insertion(items[0], ''), 'item');
    assert.equal(features.insertion(items[0], '('), 'item');
    assert.equal(local.signature, undefined, 'presentation does not mutate compiler metadata');

    const withoutGlobal = { locals: [local] };
    assert.equal(features.completionCandidates(withoutGlobal, text, start + 2, low, source)
      .find(item => item.name === 'item').signature, hover.signature);
    for (const context of [{ ...source, saved: true }, { file: `/project/other.${suffix}` }, {}]) {
      assert.equal(features.hoverAt(withoutGlobal, text, start + 2, context), undefined);
      assert.ok(!features.completionCandidates(withoutGlobal, text, start + 2, low, context).some(item => item.name === 'item'));
    }
    assert.equal(features.hoverAt(withoutGlobal, text, text.lastIndexOf('item') + 2, source), undefined,
      'another occurrence requires its own borrowed metadata');
  });

  test(`${suffix} owned and read-only-only locals retain their existing presentation`, () => {
    const text = 'print(item)';
    const file = `/project/main.${suffix}`;
    for (const flags of [{}, { readonly: true }, { borrowed: false, readonly: true }]) {
      const local = { name: 'item', type: 'Entry', location: { file, line: 1, column: 7, length: 4 }, ...flags };
      const index = { locals: [local] };
      assert.equal(features.hoverAt(index, text, 8, { file }).item.signature, 'item: Entry');
      const completion = features.completionCandidates(index, text, 8, low, { file }).find(item => item.name === 'item');
      assert.equal(completion.signature, 'item: Entry');
      assert.equal(completion.kind, 'variable');
      assert.equal(features.insertion(completion, ''), 'item');
    }
  });

  test(`${suffix} borrowed receiver fields preserve the compiler read-only contract`, () => {
    const file = `/project/main.${suffix}`;
    for (const [text, receiver, column] of [['item.name', 'Entry', 1], ['item.inner.name', 'Inner', 1]]) {
      const member = text.lastIndexOf('.');
      const expressions = [{ location: { file, line: 1, column }, end_line: 1, end_column: member + 1,
        type: receiver, borrowed: true, fields: [{ name: 'name', type: 'str', readonly: true }] }];
      const index = { expressions };
      const hover = features.hoverAt(index, text, member + 3, { file }).item;
      assert.equal(hover.signature, 'name: str (read-only)');
      assert.equal(hover.readonly, true);
      assert.equal(hover.kind, 'field');
      const completion = features.completionCandidates(index, text, member + 3, low, { file })[0];
      assert.equal(completion.signature, hover.signature);
      assert.equal(features.insertion(completion, ''), 'name');
      const owned = { expressions: [{ ...expressions[0], borrowed: undefined, fields: [{ name: 'name', type: 'str' }] }] };
      assert.equal(features.hoverAt(owned, text, member + 3, { file }).item.signature, 'name: str');
      assert.equal(features.completionCandidates(index, text, member + 3, low, { file, saved: true }).length, 0);
    }
  });
}
