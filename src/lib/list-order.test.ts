import { describe, expect, it } from 'vitest';
import { moveItem, removeAt } from '$lib/list-order';

describe('list ordering', () => {
  it('moves and removes items without mutating the source', () => {
    const source = ['a', 'b', 'c'];
    expect(moveItem(source, 2, 0)).toEqual(['c', 'a', 'b']);
    expect(removeAt(source, 1)).toEqual(['a', 'c']);
    expect(source).toEqual(['a', 'b', 'c']);
  });
});
