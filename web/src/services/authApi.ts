import { httpClient } from './client';
import { ApiResponse, LoginResponseData, User } from '../types/auth';

export const authApi = {
  login(username: string, password: string):Promise<ApiResponse<LoginResponseData>> {
    return httpClient.request<LoginResponseData>('/api/auth/login', {
      method: 'POST',
      body: JSON.stringify({ username, password }),
    });
  },

  logout(): Promise<ApiResponse<null>> {
    return httpClient.request<null>('/api/auth/logout', {
      method: 'POST',
    });
  },

  getMe(): Promise<ApiResponse<User>> {
    return httpClient.request<User>('/api/auth/me', {
      method: 'GET',
    });
  },
};
