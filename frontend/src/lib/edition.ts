// ── Edition gate ─────────────────────────────────────────────────────────
//
// 灵枢有两个发行形态，共用同一份前端代码：
//
//   client —— 桌面记忆客户端。主流程为记忆管理 / MCP 接入 / 基础聊天，
//             模型、隐私与可选云同步作为辅助设置。
//   dev    —— 开发版（Docker 一键环境，浏览器访问 127.0.0.1:5173）。
//             额外暴露 Memory Console（动态记忆引擎）等调试与演示页面。
//
// 判定顺序：
//   1. 构建期变量 VITE_LINGSHU_EDITION（compose / Dockerfile.dev 里显式设为 dev）；
//   2. 构建模式 --mode client（vite.config.ts 会同时把 Memory Console 替换成空桩，
//      产物里根本不含那段代码，见 frontend/vite.config.ts）；
//   3. 运行时宿主——Tauri webview 即客户端，浏览器即开发版。
//
// 这样桌面客户端无论怎么构建都不会出现开发调试页。

import { isTauri } from './tauri';

export type Edition = 'client' | 'dev';

// 写成「三元表达式 + 字面量比较」而不是函数，是为了让打包器在客户端构建里
// 直接折叠成常量 false，从而把 Memory Console 的分支和文案一起摇掉。
// VITE_LINGSHU_EDITION / MODE 都会在构建期被替换成字面量。
const CONFIGURED_EDITION = import.meta.env.VITE_LINGSHU_EDITION;
const BUILD_MODE = import.meta.env.MODE;

/** 当前是否为开发版（Docker + 浏览器）。 */
export const isDevEdition: boolean =
  CONFIGURED_EDITION === 'dev'
    ? true
    : CONFIGURED_EDITION === 'client'
      ? false
      : BUILD_MODE === 'client'
        ? false
        : !isTauri();

export const edition: Edition = isDevEdition ? 'dev' : 'client';

/** Memory Console（动态记忆引擎）只在开发版出现。 */
export const isMemoryLabAvailable = isDevEdition;
