import { describe, expect, it } from 'vitest';
import { validateComputerQuery, validateLocalDateTime } from '../validationService';

describe('validateComputerQuery', () => {
  it('accepts NetBIOS names, FQDN and IP addresses', () => {
    for (const good of [
      'WS01',
      'kma-w0001',
      'WS01$',
      'ws01.company.ru',
      'WS01.COMPANY.RU.',
      '10.0.0.5',
      '192.168.122.255',
      '2001:db8::1',
    ]) {
      expect(validateComputerQuery(good), good).toEqual({ valid: true });
    }
  });

  it('rejects empty, overlong and hostile input', () => {
    for (const bad of [
      '',
      '   ',
      'x'.repeat(256),
      'ws 01',
      'ws*01',
      'ws01)(cn=*',
      '10.0.0.256',
      '.company.ru',
      'ws01..company.ru',
    ]) {
      expect(validateComputerQuery(bad).valid, bad).toBe(false);
    }
  });
});

describe('validateLocalDateTime', () => {
  it('accepts datetime-local strings', () => {
    expect(validateLocalDateTime('2030-01-15T09:30')).toEqual({ valid: true });
  });

  it('rejects empty and broken values', () => {
    expect(validateLocalDateTime('').valid).toBe(false);
    expect(validateLocalDateTime('не дата').valid).toBe(false);
  });
});
