import { useMemo, useEffect, useCallback, useState } from 'react';
import { oauthApi } from '@/lib/api';
import { isLocalOrLanAccess } from '@/lib/accessControl';

interface User {
  id: string;
  name: string;
  display_name: string;
  email?: string;
  avatar?: string;
}

interface AuthState {
  isSignedIn: boolean;
  isLoaded: boolean;
  isLoading: boolean;
  userId: string | null;
  user: User | null;
}

const TOKEN_STORAGE_KEY = 'casdoor_access_token';
const USER_STORAGE_KEY = 'casdoor_user';

// Parse OAuth callback from URL hash
function parseOAuthCallback(): { accessToken?: string; tokenType?: string; expiresIn?: number } | null {
  if (typeof window === 'undefined') return null;

  const hash = window.location.hash;
  if (!hash.startsWith('#/callback#')) return null;

  const params = new URLSearchParams(hash.slice(11)); // Remove '#/callback#'
  const accessToken = params.get('access_token');
  const tokenType = params.get('token_type');
  const expiresIn = params.get('expires_in');

  if (accessToken) {
    // Clear the hash after parsing
    window.history.replaceState(null, '', window.location.pathname);
    return {
      accessToken,
      tokenType: tokenType || undefined,
      expiresIn: expiresIn ? parseInt(expiresIn, 10) : undefined,
    };
  }

  return null;
}

// Store token and user info
function storeAuthData(accessToken: string, expiresIn?: number) {
  localStorage.setItem(TOKEN_STORAGE_KEY, accessToken);

  const expiresAt = expiresIn ? Date.now() + expiresIn * 1000 : null;
  if (expiresAt) {
    localStorage.setItem('casdoor_token_expires', expiresAt.toString());
  }
}

function clearAuthData() {
  localStorage.removeItem(TOKEN_STORAGE_KEY);
  localStorage.removeItem('casdoor_token_expires');
  localStorage.removeItem(USER_STORAGE_KEY);
}

function getStoredToken(): string | null {
  const token = localStorage.getItem(TOKEN_STORAGE_KEY);
  const expiresAt = localStorage.getItem('casdoor_token_expires');

  if (token && expiresAt) {
    const expiry = parseInt(expiresAt, 10);
    if (Date.now() >= expiry) {
      clearAuthData();
      return null;
    }
  }

  return token;
}

function getStoredUser(): User | null {
  const userStr = localStorage.getItem(USER_STORAGE_KEY);
  if (userStr) {
    try {
      return JSON.parse(userStr);
    } catch {
      return null;
    }
  }
  return null;
}

export function useAuth() {
  const [authState, setAuthState] = useState<AuthState>({
    isSignedIn: false,
    isLoaded: false,
    isLoading: true,
    userId: null,
    user: null,
  });

  // Check for OAuth callback on mount
  useEffect(() => {
    const callback = parseOAuthCallback();
    if (callback?.accessToken) {
      storeAuthData(callback.accessToken, callback.expiresIn);
      // Fetch user info after getting token
      fetchUserInfo(callback.accessToken).then((user) => {
        if (user) {
          localStorage.setItem(USER_STORAGE_KEY, JSON.stringify(user));
          setAuthState({
            isSignedIn: true,
            isLoaded: true,
            isLoading: false,
            userId: user.id,
            user,
          });
        }
      });
    }
  }, []);

  // Check for existing auth on mount
  useEffect(() => {
    const token = getStoredToken();
    const user = getStoredUser();

    if (token && user) {
      setAuthState({
        isSignedIn: true,
        isLoaded: true,
        isLoading: false,
        userId: user.id,
        user,
      });
    } else {
      setAuthState((prev) => ({ ...prev, isLoaded: true, isLoading: false }));
    }
  }, []);

  const login = useCallback(async () => {
    // Block login if accessing from public domain (non-local/LAN)
    if (!isLocalOrLanAccess()) {
      console.warn('Login is only allowed from local/LAN access');
      return;
    }

    try {
      const response = await oauthApi.handoffInit('casdoor', window.location.pathname);
      // Redirect to Casdoor authorization page
      window.location.href = response.authorize_url;
    } catch (error) {
      console.error('Login failed:', error);
    }
  }, []);

  const logout = useCallback(async () => {
    // Get the access token before clearing local data
    const accessToken = localStorage.getItem(TOKEN_STORAGE_KEY);

    // Clear local auth data first
    clearAuthData();
    setAuthState({
      isSignedIn: false,
      isLoaded: true,
      isLoading: false,
      userId: null,
      user: null,
    });

    try {
      // Call backend logout API with access token to get the logout URL
      // Casdoor will redirect back to the app after logout completes
      const { logout_url } = await oauthApi.logout(accessToken || undefined);

      // Redirect to Casdoor logout URL to clear the authentication server session
      window.location.href = logout_url;
    } catch (error) {
      console.error('Logout API call failed:', error);
      // Local data is already cleared, so just log the error
    }
  }, []);

  return useMemo(
    () => ({
      isSignedIn: authState.isSignedIn,
      isLoaded: authState.isLoaded,
      isLoading: authState.isLoading,
      userId: authState.userId,
      user: authState.user,
      profile: authState.user, // For backwards compatibility
      login,
      logout,
    }),
    [authState, login, logout]
  );
}

async function fetchUserInfo(accessToken: string): Promise<User | null> {
  try {
    // Decode JWT to get user info (simple approach)
    const parts = accessToken.split('.');
    if (parts.length === 3) {
      const payload = JSON.parse(atob(parts[1]));
      return {
        id: payload.sub || payload.id || '',
        name: payload.preferred_username || payload.name || '',
        display_name: payload.name || payload.display_name || '',
        email: payload.email,
        avatar: payload.picture,
      };
    }
  } catch (error) {
    console.error('Failed to decode token:', error);
  }
  return null;
}
