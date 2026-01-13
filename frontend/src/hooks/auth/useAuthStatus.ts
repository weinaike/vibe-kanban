// Stub for local-only mode (OAuth removed)
import { useMemo } from 'react';

export function useAuthStatus() {
  return useMemo(
    () => ({
      isSignedIn: false,
      isLoading: false,
    }),
    []
  );
}
