// Simplified version for local-only mode (Electric sync removed)

export const createAuthenticatedShapeOptions = (table: string) => ({
  url: `/api/electric/shape/${table}`,
  headers: {},
  parser: {
    timestamptz: (value: string) => value,
  },
});
