export const SAVANT_APP_HEADERS = { 'X-App-Name': 'savant-sanctum' } as const;

export function buildSavantHeaders(apiKey = '', json = false): Record<string, string> {
  return {
    ...SAVANT_APP_HEADERS,
    ...(json ? { 'Content-Type': 'application/json' } : {}),
    ...(apiKey ? { 'X-API-Key': apiKey } : {}),
  };
}
