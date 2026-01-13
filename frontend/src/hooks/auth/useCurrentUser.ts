// Stub for local-only mode (OAuth removed)
import { useMemo } from 'react';

export function useCurrentUser() {
  return useMemo(() => ({ user: null, isLoading: false }), []);
}
