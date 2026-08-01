import { describe, expect, it } from 'vitest';
import { buildSavantHeaders } from '../services/httpClient';

describe('buildSavantHeaders', () => {
  it('identifies Sanctum on unauthenticated service requests', () => {
    expect(buildSavantHeaders()).toEqual({ 'X-App-Name': 'savant-sanctum' });
  });

  it('adds authentication and JSON headers for Savant API writes', () => {
    expect(buildSavantHeaders('secret', true)).toEqual({
      'X-App-Name': 'savant-sanctum',
      'Content-Type': 'application/json',
      'X-API-Key': 'secret',
    });
  });
});
