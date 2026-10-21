import React from 'react';
import ReactDOM from 'react-dom/client';
import { AuthProvider } from './context/AuthContext';
import { ToastProvider } from './context/ToastContext';
import { App } from './App';

// Import CSS Design System
import './styles/tokens.css';
import './styles/layout.css';
import './styles/components.css';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <AuthProvider>
      <ToastProvider>
        <App />
      </ToastProvider>
    </AuthProvider>
  </React.StrictMode>
);
