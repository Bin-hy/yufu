<script setup lang="ts">
import { useId } from 'vue'

/**
 * yufu 鱼符标志。
 *
 * 一条竖向的鱼符：头朝上、尾叉朝下，中缝横贯鱼身。
 * 「分开是凭证，合起来才是身份」——这正是认证本身的动作。
 *
 * 几何唯一真源在 brand/generate.py，改动请先改那里再同步到这里。
 * 32px 以下省略眼与穿绳孔：那个尺度下它们会糊成噪点（favicon 做了同样处理）。
 */
withDefaults(defineProps<{ size?: number }>(), { size: 18 })

// 多处复用同一个标志时，defs 里的 id 必须唯一
const uid = useId()

// 基础鱼形：鼻尖在左、叉尾在右；旋转 90° 后即为竖鱼
const FISH =
    'M 8.4 32 C 11.6 27.2 15.8 21.4 23.4 20.6 L 42.4 27.6 L 55.5 19.5 ' +
    'L 48.6 32 L 55.5 44.5 L 42.4 36.4 L 23.4 43.4 C 15.8 42.6 11.6 36.8 8.4 32 Z'
</script>

<template>
    <svg
        :width="size * 0.532"
        :height="size"
        viewBox="14.7 -0.6 34.6 65"
        fill="currentColor"
        aria-hidden="true"
        focusable="false"
    >
        <defs>
            <path :id="`yufu-fish-${uid}`" fill-rule="evenodd" :d="FISH" />
            <clipPath :id="`yufu-head-${uid}`">
                <rect x="0" y="0" width="30.6" height="64" />
            </clipPath>
            <clipPath :id="`yufu-tail-${uid}`">
                <rect x="33.4" y="0" width="30.6" height="64" />
            </clipPath>
        </defs>
        <!-- 旋转后中缝随之转为横贯鱼身 -->
        <g transform="rotate(90 32 32) translate(32 32) scale(1.38) translate(-32 -32)">
            <use :href="`#yufu-fish-${uid}`" :clip-path="`url(#yufu-head-${uid})`" />
            <use :href="`#yufu-fish-${uid}`" :clip-path="`url(#yufu-tail-${uid})`" />
        </g>
    </svg>
</template>
