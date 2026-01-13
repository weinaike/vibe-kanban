// Stub for local-only mode (OAuth removed)
import { useMemo } from 'react';

export function useAuth() {
  return useMemo(
    () => ({
      isSignedIn: false,
      isLoaded: true,
      profile: null,
      isLoading: false,
      userId: null as string | null,
    }),
    []
  );
}
