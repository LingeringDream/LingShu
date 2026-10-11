/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** 发行形态：client = 桌面客户端，dev = 开发版。留空则按运行时宿主自动判定。 */
  readonly VITE_LINGSHU_EDITION?: string;
}
