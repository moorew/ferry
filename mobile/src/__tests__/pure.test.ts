import { expect, test } from '@jest/globals';

import { age } from '../format';
import { linkFrom } from '../pairing';
import { t } from '../strings';

test('strings fill placeholders', () => {
  expect(t('row.from', { device: 'desk' })).toBe('From desk');
  expect(t('search.empty', { query: 'x' })).toBe('Nothing matches “x”');
});

test('ages read naturally', () => {
  const now = 1_800_000_000_000;
  expect(age(now - 10_000, now)).toBe('just now');
  expect(age(now - 5 * 60_000, now)).toBe('5 min ago');
  expect(age(now - 3 * 3_600_000, now)).toBe('3 h ago');
  expect(age(now - 2 * 86_400_000, now)).toBe('2 d ago');
});

test('a pairing deep link rebuilds with every address', () => {
  const link = linkFrom({ v: '1', id: 'd', name: 'Desk & co', key: 'abc', addr: ['100.64.0.1:7441', '[fd7a::1]:7441'] });
  expect(link).toBe('tailcopy://pair?v=1&id=d&name=Desk+%26+co&key=abc&addr=100.64.0.1%3A7441&addr=%5Bfd7a%3A%3A1%5D%3A7441');
  expect(linkFrom({ id: 'd' })).toBeNull();
});
