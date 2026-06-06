// ── Pet Window React Entry ─────────────────────────────────────────────
// This is the small frameless desktop pet. It renders the avatar placeholder
// and allows dragging via the Tauri data-tauri-drag-region attribute.
//
// In the browser (non-Tauri), this renders the full app since there's no
// Tauri window to differentiate. Use a URL param or Tauri API to decide.

import React from 'react';
import ReactDOM from 'react-dom/client';
import { PetWindow } from './components/avatar/PetWindow';
import './index.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <PetWindow />
  </React.StrictMode>
);
