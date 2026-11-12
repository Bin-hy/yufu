#!/usr/bin/env python3
"""yufu 鱼符 · 品牌资产生成器

单一几何来源，生成全部 SVG 资产。改这里的数字即可重制全套。
运行：python3 brand/generate.py

定版：一条竖鱼（头朝上、尾叉朝下），中缝横贯鱼身。
  · 主标 = 无框竖鱼
  · 应用图标 = 同一条竖鱼反白入墨印方框
中缝永不可省：分开是凭证，合起来才是身份。
"""
import os

OUT = os.path.dirname(os.path.abspath(__file__))

# ---------------------------------------------------------------- 几何
# 基础鱼形：鼻尖在左(8.4,32)，叉尾在右；眼与穿绳孔以 evenodd 挖空。
# 竖鱼 = 该鱼形旋转 90°（鼻尖朝上）。
FISH = (
    "M 8.4 32 "
    "C 11.6 27.2 15.8 21.4 23.4 20.6 "   # 头背弧
    "L 42.4 27.6 "                        # 背线至尾柄
    "L 55.5 19.5 "                        # 尾鳍上叶
    "L 48.6 32 "                          # 尾叉
    "L 55.5 44.5 "                        # 尾鳍下叶
    "L 42.4 36.4 "                        # 尾柄下缘
    "L 23.4 43.4 "                        # 腹线
    "C 15.8 42.6 11.6 36.8 8.4 32 Z "     # 头腹弧回到鼻尖
)
EYE = "M 16.6 28.6 a 2.1 2.1 0 1 0 4.2 0 a 2.1 2.1 0 1 0 -4.2 0 Z"
HOLE = "M 37.75 25.15 a 1.35 1.35 0 1 0 2.7 0 a 1.35 1.35 0 1 0 -2.7 0 Z"

SEAM_GAP = 2.8        # 标准中缝
SMALL_GAP = 4.6       # 小尺寸光学补偿：缝更宽才不会糊成一片
FISH_SCALE = 1.38     # 竖鱼在 64 见方内的放大
SMALL_SCALE = 1.30    # favicon 略收，给加宽的缝留出空间

# 竖鱼旋转后在 64 坐标系里的实际包围盒（用于收紧画板）
VB_X, VB_Y, VB_W, VB_H = 14.70, -0.60, 34.60, 65.00

# ---------------------------------------------------------------- 色板
INK = "#101820"        # 墨（主色 / 文字）
JADE = "#0E7C66"       # 玉（品牌主色）
JADE_HI = "#3FC7A4"    # 玉·暗底提亮
PAPER = "#F2F5F8"      # 反白
VERMILION = "#B4382C"  # 朱（点色，仅用于强调）
GRAY = "#6B7784"

# 定版字体：Futura。macOS 的 "Futura" 即 Futura Medium。
FONT = "'Futura','Futura-Medium','Avenir Next','Helvetica Neue',Arial,sans-serif"
CN_FONT = "'PingFang SC','Noto Sans SC','Hiragino Sans GB',sans-serif"


# ---------------------------------------------------------------- 构件
def fish_path(detail=True):
    return FISH + (EYE + (HOLE if detail else ""))


def halves(fill_l, fill_r, detail=True, uid="h", gap=SEAM_GAP):
    """鱼的左右两半，各自被中缝裁开 —— 这里的「左右」在旋转后即为「上下」。"""
    l, r = 32 - gap / 2, 32 + gap / 2
    return (
        f'<defs><path id="fish-{uid}" fill-rule="evenodd" d="{fish_path(detail)}"/>'
        f'<clipPath id="hl-{uid}"><rect x="0" y="0" width="{l:.2f}" height="64"/></clipPath>'
        f'<clipPath id="hr-{uid}"><rect x="{r:.2f}" y="0" width="{64 - r:.2f}" height="64"/></clipPath>'
        f"</defs>"
        f'<use href="#fish-{uid}" fill="{fill_l}" clip-path="url(#hl-{uid})"/>'
        f'<use href="#fish-{uid}" fill="{fill_r}" clip-path="url(#hr-{uid})"/>'
    )


def vfish(fill_l, fill_r, uid, detail=True, gap=SEAM_GAP, scale=FISH_SCALE):
    """定版竖鱼：头朝上、尾叉朝下、中缝横贯。坐标仍在 64 见方内。"""
    t = f"rotate(90 32 32) translate(32,32) scale({scale}) translate(-32,-32)"
    return f'<g transform="{t}">{halves(fill_l, fill_r, detail, uid, gap)}</g>'


def standalone(body, vb, w, h, label="yufu 鱼符"):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb}" width="{w}" height="{h}" '
        f'role="img" aria-label="{label}">\n<title>{label}</title>\n{body}\n</svg>\n'
    )


TIGHT = f"{VB_X} {VB_Y} {VB_W} {VB_H}"          # 收紧到鱼身（34.6 × 65）
TW, TH = 35, 66                                  # 对应输出尺寸


def mark(fill_l, fill_r, uid, flip=False):
    """主标文件：画板收紧到鱼身，不含印框。flip=True 转回横鱼（归档用）。"""
    body = halves(fill_l, fill_r, uid=uid)
    if flip:
        return standalone(body, "0 0 64 64", 64, 64)
    return standalone(vfish(fill_l, fill_r, uid), TIGHT, TW, TH)


def seal(bg, fg, uid, rx=14, gap=SEAM_GAP, detail=True, scale=FISH_SCALE):
    """应用图标：同一条竖鱼反白入墨印方框。"""
    return (
        f'<rect x="0" y="0" width="64" height="64" rx="{rx}" fill="{bg}"/>'
        + vfish(fg, fg, uid, detail, gap, scale)
    )


ASSETS = {}

# ---------------------------------------------------------------- 主标（竖鱼，无框）
ASSETS["logo-mark.svg"] = mark(INK, INK, "m")
ASSETS["logo-mark-reverse.svg"] = mark(PAPER, PAPER, "r")
ASSETS["logo-mark-duplex.svg"] = mark(JADE, INK, "d")
ASSETS["logo-mark-duplex-reverse.svg"] = mark(JADE_HI, PAPER, "dr")
ASSETS["logo-mark-mono.svg"] = mark("currentColor", "currentColor", "c")

# ---------------------------------------------------------------- 应用图标（竖鱼入墨印）
ASSETS["app-icon.svg"] = standalone(seal(INK, "#FFFFFF", "ai"), "0 0 64 64", 1024, 1024)
ASSETS["app-icon-jade.svg"] = standalone(seal(JADE, "#FFFFFF", "aj"), "0 0 64 64", 1024, 1024)
ASSETS["favicon.svg"] = standalone(
    seal(INK, "#FFFFFF", "fv", rx=13, gap=SMALL_GAP, detail=False, scale=SMALL_SCALE),
    "0 0 64 64", 64, 64,
)

# ---------------------------------------------------------------- 字标组合（Futura）
# 横版：竖鱼置于字标左侧，鱼高 56 / 画板 64；字标 40px
# 主标高 44（≈1.1 × 字标字高），与 40px 的 yufu 配平
_MK = dict(s=0.677, tx=-9.95, ty=10.40)   # s = 44 / 65
_LOCKUP_MARK = (
    f'<g transform="translate({_MK["tx"]},{_MK["ty"]}) scale({_MK["s"]})">'
    + vfish(INK, INK, "lh")
    + "</g>"
)
_LOCKUP_MARK_R = (
    f'<g transform="translate({_MK["tx"]},{_MK["ty"]}) scale({_MK["s"]})">'
    + vfish(PAPER, PAPER, "lhr")
    + "</g>"
)
ASSETS["logo-lockup-horizontal.svg"] = (
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 64" width="240" height="128" '
    'role="img" aria-label="yufu 鱼符">\n<title>yufu · 鱼符</title>\n'
    + _LOCKUP_MARK
    + f'<text x="38" y="46" font-family="{FONT}" font-size="40" font-weight="500" '
      f'letter-spacing="-1" fill="{INK}">yufu</text>\n</svg>\n'
)
ASSETS["logo-lockup-horizontal-reverse.svg"] = (
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 64" width="240" height="128" '
    'role="img" aria-label="yufu 鱼符">\n<title>yufu · 鱼符</title>\n'
    + _LOCKUP_MARK_R
    + f'<text x="38" y="46" font-family="{FONT}" font-size="40" font-weight="500" '
      f'letter-spacing="-1" fill="{PAPER}">yufu</text>\n</svg>\n'
)
# 竖版组合：竖鱼在上 + 字标 + 中文
ASSETS["logo-lockup-vertical.svg"] = (
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 140 146" width="280" height="292" '
    'role="img" aria-label="yufu 鱼符">\n<title>yufu · 鱼符</title>\n'
    '<g transform="translate(42.4,0.52) scale(0.8615)">' + vfish(INK, INK, "lv") + "</g>"
    + f'<text x="70" y="98" text-anchor="middle" font-family="{FONT}" font-size="38" '
      f'font-weight="500" letter-spacing="-1" fill="{INK}">yufu</text>'
    + f'<text x="70" y="130" text-anchor="middle" font-family="{CN_FONT}" font-size="17" '
      f'letter-spacing="7" fill="{GRAY}">鱼符</text>\n</svg>\n'
)

# ---------------------------------------------------------------- 归档：横鱼（备选，不对外主推）
ASSETS["alt/logo-mark-wide.svg"] = mark(INK, INK, "mw", flip=True)
ASSETS["alt/logo-mark-wide-duplex.svg"] = mark(JADE, INK, "mwd", flip=True)

if __name__ == "__main__":
    os.makedirs(os.path.join(OUT, "alt"), exist_ok=True)
    for name, svg in ASSETS.items():
        path = os.path.join(OUT, name)
        with open(path, "w", encoding="utf-8") as f:
            f.write(svg)
        print(f"  {name:38s} {len(svg):5d} B")
    print(f"\n共生成 {len(ASSETS)} 个资产 → {OUT}")
