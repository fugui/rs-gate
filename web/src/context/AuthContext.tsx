import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';
import { User, UserRole } from '../types/auth';
import { httpClient } from '../services/client';
import { authApi } from '../services/authApi';

interface AuthContextType {
  user: User | null;
  token: string | null;
  role: UserRole | null;
  isAuthenticated: boolean;
  isAdmin: boolean;
  isSuperAdmin: boolean;
  login: (token: string, user: User) => void;
  logout: () => Promise<void>;
}

const AuthContext = createContext<AuthContextType | null>(null);

export const AuthProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [token, setTokenState] = useState<string | null>(() => localStorage.getItem('rsgate_token'));
  const [user, setUserState] = useState<User | null>(() => {
    try {
      const stored = localStorage.getItem('rsgate_user');
      return stored ? JSON.parse(stored) : null;
    } catch {
      return null;
    }
  });

  const login = useCallback((newToken: string, newUser: User) => {
    httpClient.setToken(newToken);
    localStorage.setItem('rsgate_user', JSON.stringify(newUser));
    setTokenState(newToken);
    setUserState(newUser);
  }, []);

  const logout = useCallback(async () => {
    try {
      if (token) {
        await authApi.logout();
      }
    } catch {
      // ignore logout network errors
    } finally {
      httpClient.setToken(null);
      localStorage.removeItem('rsgate_user');
      setTokenState(null);
      setUserState(null);
      window.location.hash = '#/login';
    }
  }, [token]);

  useEffect(() => {
    const handleUnauthorized = () => {
      logout();
    };

    window.addEventListener('unauthorized', handleUnauthorized);
    return () => {
      window.removeEventListener('unauthorized', handleUnauthorized);
    };
  }, [logout]);

  const role = user?.role || null;
  const isAuthenticated = !!token && !!user;
  const isAdmin = role === 'admin' || role === 'superadmin';
  const isSuperAdmin = role === 'superadmin';

  return (
    <AuthContext.Provider
      value={{
        user,
        token,
        role,
        isAuthenticated,
        isAdmin,
        isSuperAdmin,
        login,
        logout,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
};

export const useAuth = () => {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error('useAuth must be used within AuthProvider');
  }
  return ctx;
};
