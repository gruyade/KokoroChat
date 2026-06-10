import { describe, it, expect } from 'vitest';
import * as fc from 'fast-check';

import { formatSize } from './KnowledgeSection';

/**
 * Property 13: Size formatting correctness
 *
 * For any non-negative integer size_bytes value, the human-readable format function
 * SHALL produce a string with the correct unit (bytes for <1024, KB for <1048576, MB otherwise)
 * and a numerically accurate representation.
 *
 * Feature: knowledge-plugin, Property 13: Size formatting correctness
 *
 * **Validates: Requirements 8.3**
 */
describe('formatSize - Property 13: Size formatting correctness', () => {
  /**
   * Property 13a: bytes < 1024 の場合、"{bytes} bytes" を返す
   */
  it('property: values < 1024 return "{n} bytes" format', () => {
    fc.assert(
      fc.property(
        fc.integer({ min: 0, max: 1023 }),
        (bytes) => {
          const result = formatSize(bytes);
          expect(result).toBe(`${bytes} bytes`);
        }
      ),
      { numRuns: 200 }
    );
  });

  /**
   * Property 13b: 1024 <= bytes < 1048576 の場合、正確な KB 表記を返す
   */
  it('property: values in [1024, 1048576) return correct KB format', () => {
    fc.assert(
      fc.property(
        fc.integer({ min: 1024, max: 1048575 }),
        (bytes) => {
          const result = formatSize(bytes);
          const expectedValue = (bytes / 1024).toFixed(1);
          expect(result).toBe(`${expectedValue} KB`);
        }
      ),
      { numRuns: 200 }
    );
  });

  /**
   * Property 13c: bytes >= 1048576 の場合、正確な MB 表記を返す
   */
  it('property: values >= 1048576 return correct MB format', () => {
    fc.assert(
      fc.property(
        fc.integer({ min: 1048576, max: 1073741824 }), // up to 1 GB
        (bytes) => {
          const result = formatSize(bytes);
          const expectedValue = (bytes / 1048576).toFixed(1);
          expect(result).toBe(`${expectedValue} MB`);
        }
      ),
      { numRuns: 200 }
    );
  });

  /**
   * Property 13d: 単位の正しさ — 正しいサフィックスが付いている
   */
  it('property: output always ends with correct unit suffix', () => {
    fc.assert(
      fc.property(
        fc.integer({ min: 0, max: 1073741824 }),
        (bytes) => {
          const result = formatSize(bytes);
          if (bytes < 1024) {
            expect(result).toMatch(/ bytes$/);
          } else if (bytes < 1048576) {
            expect(result).toMatch(/ KB$/);
          } else {
            expect(result).toMatch(/ MB$/);
          }
        }
      ),
      { numRuns: 200 }
    );
  });

  /**
   * 境界値のユニットテスト — 単位の遷移ポイントを明示的に検証
   */
  it('boundary: 0 bytes', () => {
    expect(formatSize(0)).toBe('0 bytes');
  });

  it('boundary: 1023 bytes (max bytes range)', () => {
    expect(formatSize(1023)).toBe('1023 bytes');
  });

  it('boundary: 1024 bytes (transition to KB)', () => {
    expect(formatSize(1024)).toBe('1.0 KB');
  });

  it('boundary: 1048575 bytes (max KB range)', () => {
    expect(formatSize(1048575)).toBe(`${(1048575 / 1024).toFixed(1)} KB`);
  });

  it('boundary: 1048576 bytes (transition to MB)', () => {
    expect(formatSize(1048576)).toBe('1.0 MB');
  });

  it('boundary: large MB value', () => {
    const bytes = 10485760; // 10 MB
    expect(formatSize(bytes)).toBe('10.0 MB');
  });
});
