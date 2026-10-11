import { defineConfig, loadEnv, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

// Memory Console（动态记忆引擎）只在开发版 / 竞赛版出现。客户端构建
// (`npm run build:client`，即 --mode client) 会把它替换成空桩模块，
// 这样它的组件、样式和依赖都不会进入客户端产物。
const MEMORY_LAB_STUB_ID = '\0lingshu:memory-lab-stub';

function memoryLabStubPlugin(enabled: boolean): Plugin {
  return {
    name: 'lingshu-memory-lab-stub',
    enforce: 'pre',
    resolveId(source) {
      if (!enabled) return null;
      return /components\/memory\/MemoryEvolutionLab(\.tsx)?$/.test(source) ? MEMORY_LAB_STUB_ID : null;
    },
    load(id) {
      if (id !== MEMORY_LAB_STUB_ID) return null;
      return [
        'function MemoryEvolutionLabStub() { return null; }',
        'export const MemoryEvolutionLab = MemoryEvolutionLabStub;',
        'export default MemoryEvolutionLabStub;',
      ].join('\n');
    },
  };
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  const isClientBuild =
    mode === 'client' || String(env.VITE_LINGSHU_EDITION ?? '').trim().toLowerCase() === 'client';
  // Default to 127.0.0.1 (not "localhost") to match the backend's IPv4 bind
  // and avoid the proxy resolving to IPv6 ::1 where nothing is listening.
  const apiTarget = env.VITE_API_URL || 'http://127.0.0.1:8080';
  // /^https?/ → 'ws$1' correctly maps http→ws and https→wss.
  // The naive /^http/ → 'ws' would turn https:// into wsss://.
  const wsTarget = apiTarget.replace(/^https?/, (m) => (m === 'https' ? 'wss' : 'ws'));

  return {
    plugins: [react(), memoryLabStubPlugin(isClientBuild)],
    // 客户端构建把发行形态固化成编译期常量，配合上面的空桩，
    // 让 Memory Console 相关的分支与文案一起被摇掉。
    define: isClientBuild ? { 'import.meta.env.VITE_LINGSHU_EDITION': JSON.stringify('client') } : {},
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
          // 桌宠仅保留开发入口，不进入个人记忆客户端产物。
          ...(!isClientBuild ? { pet: path.resolve(__dirname, 'pet.html') } : {}),
        },
      },
    },
    // Prevent Vite from obscuring Rust errors
    clearScreen: false,
  };
});
