import { describe, it, expect } from 'vitest';
import { otherVersions, versionValidFrom } from './lawVersions';

const version = (date: string, quote = "'") => `$id: wet_a\nvalid_from: ${quote}${date}${quote}\narticles:\n  - number: '1'\n    valid_from: '1999-01-01'\n`;

describe('versionValidFrom', () => {
  it('reads the top-level date, quoted or not, and not an article\'s', () => {
    expect(versionValidFrom(version('2025-01-01'))).toBe('2025-01-01');
    expect(versionValidFrom(version('2024-01-01', ''))).toBe('2024-01-01');
    expect(versionValidFrom('$id: wet_a\narticles: []\n')).toBeNull();
  });
});

describe('otherVersions', () => {
  const v2024 = version('2024-01-01');
  const v2025 = version('2025-01-01');

  it('leaves out the saved copy of the version being edited', () => {
    expect(otherVersions([v2024, v2025], '2025-01-01')).toEqual([v2024]);
  });

  it('keeps them all without a date to leave out', () => {
    expect(otherVersions([v2024, v2025], null)).toEqual([v2024, v2025]);
  });
});
