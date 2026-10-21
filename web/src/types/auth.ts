export type UserRole = 'superadmin' | 'admin' | 'user';

export interface User {
  id: string;
  username: string;
  display_name: string;
  email?: string;
  role: UserRole;
  created_at: number;
}

export interface LoginResponseData {
  token: string;
  expires_at: number;
  user: User;
}

export interface ApiResponse<T = unknown> {
  code: number;
  message: string;
  data: T;
}

export interface PaginatedData<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
  has_more?: boolean;
}
