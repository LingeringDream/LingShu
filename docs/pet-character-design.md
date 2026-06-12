# 灵枢宠物形象设计文档

> 基于当前代码实现整理，记录「星核」角色的视觉系统、动画逻辑与扩展规范。

---

## 1. 设计理念

**定位**：浮动桌面精灵，非具象生物，以「意识球体」的视觉语言体现灵枢的 AI 身份。  
**代号**：**星核（Star Core）**  
**核心意象**：行星内核 + 轨道环 + 传感器眼睛 ── 科技感、有机感并存，避免卡通脸谱化。

---

## 2. 视觉层结构

窗口尺寸 200×260 px；角色中心固定在 (100, 110)，PixiJS Canvas 内坐标系原点在画布左上角。

角色由 11 个 `Graphics` 图层叠加渲染，从后到前顺序：

| 层级 | 名称 | 作用 |
|------|------|------|
| 1 | `glow` | 模糊光晕，强度 18，随心情变色 |
| 2 | `orbitBack` | 轨道环后半段（α=0.22，线宽 1.5） |
| 3 | `innerOrbit` | 内轨道光点 ×2（相位差 π，alpha 随相位起伏制造前后深度感） |
| 4 | `shell` | 外壳大圆（半径 38，填充 α=0.3）+ 白色描边 |
| 5 | `ring` | 内环（半径 29，描边 α=0.42，线宽 2） |
| 6 | `core` | 内核双层（半径 21 白色 + 半径 13 发光色） |
| 7 | `star` | 中央八角星（宽 14，thinking 时 17） |
| 8 | `sensor` | 传感器「眼睛」（追踪鼠标，idle r=4.5，sleepy r=3） |
| 9 | `wake` | 激活波纹（speaking / happy 时显示双弧） |
| 10 | `orbitFront` | 轨道环前半段（α=0.52，线宽 2.2） |
| 11 | `particles` | 点击爆发粒子（`burst()` 一次发射 10 颗，带重力衰减） |

### 命中区域（无点击穿透）

```
角色主体圆：圆心 (100, 110)，半径 64 px
对话气泡区：rect(6, 134, 188, 108)
```

窗口其余透明区域点击穿透到底层桌面。

---

## 3. 动画系统

### 3.1 基础参数

| 变量 | 更新规则 | 作用 |
|------|---------|------|
| `animTime` | `+= dt * 0.05`（每帧） | 全局时间轴，驱动所有周期函数 |
| `bob` (b) | `sin(animTime) * 3` | 垂直漂浮，幅度 ±3 px |
| `pulse` | `1 + sin(animTime*1.4) * 0.025 * visual.pulse` | 呼吸缩放，心情系数调幅 |
| `orbitAngle` | `+= dt * 0.05 * visual.orbitSpeed`（增量积分） | 轨道旋转角；增量式累加保证速率渐变时角度连续不跳变（内轨道光点相位 = `orbitAngle * 1.8`） |

### 3.2 挤压/弹跳（Scale 弹簧）

```
sc   ── 当前缩放（弹簧输出）
tsc  ── 目标缩放（触发点）
sc += (tsc - sc) * 0.12   // 每帧收敛
if |tsc - sc| < 0.002 → tsc = 1   // 自动回弹
```

触发时机：

| 方法 | tsc | 触发场景 |
|------|-----|---------|
| `bounce()` | 1.2 | 被点击 |
| `squish()` | 0.85 | 鼠标按下 |
| `relax()` | 1.15 | 鼠标释放 |
| `setMood()` | 1.0 | 心情切换时重置 |

### 3.3 传感器（眼睛）跟随

```
tx, ty  ── 鼠标目标偏移（基于归一化鼠标坐标，[-4,+4] / [-2,+2]）
ex, ey  ── 当前偏移（低通滤波 α=0.08）
ex += (tx - ex) * 0.08
```

sleepy 心情下眼睛缩小（r=3），alpha 降至 0.45，呼应「半眯」状态。

---

## 4. 心情系统（Mood）

`petPresentation.ts` 统一管理所有心情的视觉参数，避免硬编码散落各处。

### 4.1 心情列表

| 心情 | 触发条件 | 主色 | 光晕色 | 轨道速率 | 脉冲系数 |
|------|---------|------|-------|--------|---------|
| `idle` | 默认 / 无操作 | `#2e6bff` 蓝 | `#22a8ff` | 0.45 | 1.0 |
| `thinking` | 等待 LLM 响应 | `#9b8cff` 紫 | `#5848f5` | 1.15 | 0.9 |
| `speaking` | 流式输出中 | `#6ce0a0` 绿 | `#22d3a6` | 0.85 | 1.2 |
| `happy` | 收到正面交互 | `#ffc060` 橙黄 | `#ffd48a` | 1.35 | 1.25 |
| `sleepy` | 长时间无操作 | `#8899bb` 灰蓝 | `#6b7fa3` | 0.25 | 0.75 |

### 4.2 心情切换过渡

心情切换不直接跳变：`update()` 每帧用线性插值（lerp，系数 `0.08 * dt`，约 0.5 秒收敛）把当前视觉参数缓动到目标心情——`color` / `glowColor` 按 RGB 通道分别插值，`orbitSpeed` / `pulse` 数值插值；`eyeShape` 是离散形态，立即切换。插值辅助函数 `lerpColor` / `lerpPresentation` 位于 `petPresentation.ts`。

### 4.3 心情特效附加规则

- **`thinking`**：中央星形放大（outer 17→14），轨道加速，脉冲略微收缩（0.9）——「聚焦内省」感。
- **`speaking` / `happy`**：显示 `wake` 激活双弧（模拟「嘴部」或「能量溢出」）。
- **`sleepy`**：轨道减速，脉冲最弱（0.75），传感器眼睛变小变暗。
- **`happy`**：轨道最快（1.35），脉冲最强（1.25）——「活跃跳动」感。

### 4.4 扩展心情

添加新心情只需在 `MOOD_PRESENTATION` 增加一条记录，类型系统自动保证覆盖。示例：

```ts
surprised: {
  color: 0xff6b6b,
  glowColor: 0xff9090,
  orbitSpeed: 2.0,
  pulse: 1.4,
  eyeShape: 'open',
},
```

---

## 5. 回复气泡展示规则

`getReplyDisplayTarget(reply)` 按长度路由：

| 条件 | 展示位置 |
|------|---------|
| 去空格后 ≤ 36 字符 | `bubble`：角色旁侧悬浮气泡（短暂消失） |
| 去空格后 > 36 字符 | `dialog`：角色下方对话框（持久保留） |

---

## 6. 交互行为

| 事件 | 行为 |
|------|------|
| `mousedown` 在角色体内 | `squish()`；记录拖拽起点 |
| `mousemove`（拖拽中） | 位移 > 2 px → `startDragging()` 调 Tauri 窗口拖拽 |
| `mouseup` | `bounce()` / `relax()`；未拖拽则切换对话框显示 |
| `mousemove`（非拖拽） | `lookAt(x, y)` 更新传感器目标 |
| 单击（未拖拽） | 切换对话框 open/close；Tauri 模式下可唤起主窗口 |

---

## 7. 窗口尺寸规范

```
总窗口：200 × 260 px（含对话区）
角色区：200 × 140 px（中心 y=110）
对话区：188 × 108 px，偏移 (6, 134)
```

PixiJS Canvas 渲染尺寸与窗口相同，设备像素比自适应（PixiJS 默认行为）。

---

## 8. 代码入口

| 文件 | 职责 |
|------|------|
| [`frontend/src/components/avatar/PetWindow.tsx`](../frontend/src/components/avatar/PetWindow.tsx) | 完整角色渲染、动画循环、交互响应 |
| [`frontend/src/components/avatar/petPresentation.ts`](../frontend/src/components/avatar/petPresentation.ts) | 心情视觉参数表（单一数据源） |
| [`frontend/src/components/avatar/AvatarPlaceholder.tsx`](../frontend/src/components/avatar/AvatarPlaceholder.tsx) | 主窗口内静态预览（CSS 实现，与 PixiJS 版保持视觉一致） |

---

## 9. 人格驱动的动画调制

`petPresentation.ts` 中的 `PersonalityTraits` / `PersonalityModifiers` 将后端 SoulLedger 的 7 个人格参数映射为视觉行为乘数：

| 修饰量 | 计算来源 | 影响 |
|--------|---------|------|
| `orbitSpeedMult` | proactivity + risk_tolerance | 轨道旋转速率 |
| `pulseMult` | warmth + humor | 呼吸脉冲幅度 |
| `blinkInterval` | proactivity | 眨眼频率（越活跃越频繁） |
| `idleLookFreq` | warmth + humor | 无鼠标时随机望向频率 |
| `bounceMagnitude` | warmth + humor − formality | 点击回弹幅度 |

后端 `chat.rs` 在每次对话开始时通过 WebSocket 广播 `{type:"mood", title:"thinking", data:{...PersonalityValues, has_role_prompt:bool}}`；
对话结束时广播 `{type:"mood", title:"idle"|"happy"}`（humor>0.6 时为 happy）；流式输出期间为 `"speaking"`。

前端 `PetCharacter.applyPersonality(traits)` 接收后调用 `traitsToModifiers()` 更新 `mods`，后续 `update()` 帧自动使用新乘数，无需任何额外调用。

---

## 10. 已知局限 / 待做项

- **声效/触觉**：macOS 触控板的 haptic feedback 在 Tauri 中可通过 `NSHapticFeedbackManager` 实现（L2 级可选增强）。
- **wake 波纹离散显隐**：speaking / happy 的激活双弧仍是开关式显示，未随心情过渡淡入淡出。
