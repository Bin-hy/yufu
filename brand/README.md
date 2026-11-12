# yufu · 鱼符 — 品牌资产

唐代官员随身佩带的**身份凭证**：鱼形，一分为二，合而验之；金、银、铜三品对应不同权级。

> **标志语言**：一条竖向的几何鱼符 + 一道横贯中缝。
> 分开是凭证，合起来才是身份 —— 这正是认证（authentication）本身的动作。
> 金银铜三品，正好对上产品里的角色与访问范围。

---

## 定版

| 项 | 结论 |
| --- | --- |
| 主标 | **竖鱼**（头朝上、尾叉朝下、中缝横贯），无印框 |
| 应用图标 | **同一条竖鱼**，反白入墨印方框 —— 与主标同一个形状，只是多了框 |
| 字标 | **Futura** Medium，字距 `-1` |
| 中缝 | 标准 2.8；小尺寸 4.6（光学补偿）。**不可省略** |

全系统**只有一条鱼、一个朝向**。横鱼版本已归档到 `alt/`，不对外主推。

---

## 文件

| 文件 | 画板 | 用途 |
| --- | --- | --- |
| `logo-mark.svg` | 35 × 66 | 主标，墨色。浅色背景默认用它 |
| `logo-mark-reverse.svg` | 35 × 66 | 主标反白，深色背景 |
| `logo-mark-duplex.svg` | 35 × 66 | 双色合符（上半玉、下半墨）。大尺寸、品牌展示 |
| `logo-mark-duplex-reverse.svg` | 35 × 66 | 双色，深色底版本 |
| `logo-mark-mono.svg` | 35 × 66 | 单色版，`currentColor` 继承文字色。印刷、单色场景 |
| `app-icon.svg` | 64 × 64 | **应用图标定版**：竖鱼反白入墨印，1024 |
| `app-icon-jade.svg` | 64 × 64 | 应用图标玉印，512 |
| `favicon.svg` | 64 × 64 | favicon 简化版：去眼与穿绳孔、中缝加宽到 4.6、鱼身放大 1.30 |
| `logo-lockup-horizontal.svg` / `-reverse.svg` | 120 × 64 | 横版组合：竖鱼 + `yufu`（鱼高 44 ≈ 1.1 × 字标字高） |
| `logo-lockup-vertical.svg` | 140 × 146 | 竖版组合：竖鱼 + `yufu` + 鱼符 |
| `alt/logo-mark-wide.svg`、`alt/logo-mark-wide-duplex.svg` | 64 × 64 | **归档**：横鱼版，仅作参考 |
| `png/` | — | 位图：`icon-16/32/48/64/128/180/256/512.png`、`icon-jade-512.png`、`apple-touch-icon.png`、多尺寸 `favicon.ico` |
| `preview.html` / `preview.png` | — | 品牌预览页 |

`generate.py` 是**唯一几何来源**：改里面的 `FISH` 路径、色值或 `SEAM_GAP`，重跑即重制全套。

```sh
python3 brand/generate.py
```

---

## 排版注意（重要）

主标画板**不是正方形**，是 35 × 66（约 1 : 1.9）。因此：

```html
<!-- 对 -->
<img src="logo-mark.svg" height="32" alt="yufu">

<!-- 错：会把鱼压扁 -->
<img src="logo-mark.svg" width="32" height="32" alt="yufu">
```

只给高度、宽度自适应。图标类（`app-icon*.svg`、`favicon.svg`）是正方形，可以给死宽高。

---

## 色板

| 名称 | 值 | 用途 |
| --- | --- | --- |
| 墨 | `#101820` | 主标、正文、深色底 |
| 玉 | `#0E7C66` | 品牌主色、主按钮、链接 |
| 玉·暗底 | `#3FC7A4` | 深色主题下的主色 |
| 朱 | `#B4382C` | 点色，仅用于强调与告警，不超过画面 5% |
| 纸 | `#F2F5F8` | 反白、深色底上的文字 |
| 石灰 | `#6B7784` | 次级文字 |

---

## 用法规则

**净空**：标志四周至少留出「鱼身宽度 × 0.5」的空间，不得压字、压边。

**最小尺寸**

| 形态 | 最小高度 |
| --- | --- |
| 完整主标 | 24px（16px 仅限临时场景） |
| 方印图标 | 24px |
| favicon | 用 `favicon.svg` 或 `png/favicon.ico`，不要拿 `logo-mark.svg` 缩到 16px（眼与穿绳孔会糊成噪点） |

**不要做**

- 不要旋转、拉伸、描边加粗
- 不要在鱼上加嘴、加鳞、加卡通眼
- 不要叠加渐变、投影、发光
- 不要把中缝改成斜线或曲线
- 不要把上下两半合并成一个实心鱼

---

## 字体

字标用 `Futura` Medium，字距 `-1`。

macOS 的 `Futura` 即 Futura Medium，已在 Chrome 下验证可用（`document.fonts.check('44px Futura') === true`）。字体栈为
`'Futura','Futura-Medium','Avenir Next','Helvetica Neue',Arial,sans-serif`，其余环境会回退到 Avenir Next。

⚠️ **`logo-lockup-*.svg` 里的 `yufu` 仍是 `<text>` 元素**，依赖本机字体。接生产前必须在 Figma / Illustrator 里转成轮廓路径，否则换平台渲染会飘。中文「鱼符」同理（当前 `PingFang SC`）。

---

## 待办

- [ ] 把 `logo-lockup-*.svg` 的 `<text>` 转轮廓
- [ ] 同步到 `website/` 与控制台前端（favicon、登录页、导航栏）
- [ ] 建立品牌页 `/brand`，对外说明鱼符的来历
- [ ] 若要保留横鱼（`alt/`）作为宽容器备选，需在品牌页说明，否则直接删除
