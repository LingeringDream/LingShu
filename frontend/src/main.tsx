import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './index.css';
import { isDevEdition } from './lib/edition';

// 团队兼容入口留在开发版，不加入个人客户端的加载和导航流程。
const TeamPortal = isDevEdition
  ? React.lazy(() => import('./components/memory/TeamPortal').then((module) => ({ default: module.TeamPortal })))
  : null;
const requestedTeam = new URLSearchParams(window.location.search).get('section') === 'team'
  || /^\/team\/?$/.test(window.location.pathname);

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    {TeamPortal && requestedTeam ? (
      <React.Suspense fallback={<p role="status">正在加载团队兼容入口…</p>}><TeamPortal /></React.Suspense>
    ) : <App />}
  </React.StrictMode>
);
