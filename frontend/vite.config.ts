import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  // Default to 127.0.0.1 (not "localhost") to match the backend's IPv4 bind
  // and avoid the proxy resolving to IPv6 ::1 where nothing is listening.
  const apiTarget = env.VITE_API_URL || 'http://127.0.0.1:8080';
  // /^https?/ → 'ws$1' correctly maps http→ws and https→wss.
  // The naive /^http/ → 'ws' would turn https:// into wsss://.
  const wsTarget = apiTarget.replace(/^https?/, (m) => (m === 'https' ? 'wss' : 'ws'));

  return {
    plugins: [react()],
    resolve: {
      alias: {
        '@': path.resolve(__dirname, './src'),
      },
    },
    server: {
      port: 5173,
      // Bind to loopback only — this is a macOS desktop app with no mobile clients.
      // Use host: '0.0.0.0' only if you need LAN access for device testing.
      host: '127.0.0.1',
      proxy: {
        '/api': {
          target: apiTarget,
          changeOrigin: true,
        },
        '/ws': {
          target: wsTarget,
          ws: true,
        },
      },
    },
    build: {
      rollupOptions: {
        input: {
          main: path.resolve(__dirname, 'index.html'),
          pet: path.resolve(__dirname, 'pet.html'),
        },
      },
    },
    // Prevent Vite from obscuring Rust errors
    clearScreen: false,
  };
});
