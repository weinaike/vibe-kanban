// Stub for local-only mode (OAuth removed)

export function useAuthMutations() {
  return {
    login: () => Promise.reject(new Error('OAuth removed')),
    logout: () => Promise.resolve(),
    signup: () => Promise.reject(new Error('OAuth removed')),
  };
}
