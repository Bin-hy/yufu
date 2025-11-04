<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps({
  to: {
    type: String,
    required: true
  }
});

// 简单判断是否为外部链接
const isExternalLink = computed(() => {
  return /^(https?:|mailto:|tel:)/.test(props.to);
});
</script>

<template>
  <component
    :is="isExternalLink ? 'a' : 'router-link'"
    v-bind="isExternalLink ? { href: to, target: '_blank', rel: 'noopener' } : { to }"
  >
    <slot />
  </component>
</template>
