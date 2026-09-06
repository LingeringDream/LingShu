import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './index.css';
import { TeamPortal } from './components/memory/TeamPortal';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    {new URLSearchParams(window.location.search).get('section') === 'team' || /^\/team\/?$/.test(window.location.pathname) ? <TeamPortal /> : <App />}
  </React.StrictMode>
);
