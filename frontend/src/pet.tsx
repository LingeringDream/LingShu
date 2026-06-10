// ── Pet Window React Entry ─────────────────────────────────────────────

import React from 'react';
import ReactDOM from 'react-dom/client';
import { PetWindow } from './components/avatar/PetWindow';
import './index.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <PetWindow />
  </React.StrictMode>
);
