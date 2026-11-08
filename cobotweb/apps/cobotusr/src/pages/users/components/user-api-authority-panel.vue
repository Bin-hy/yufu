<script setup lang="ts">
import Panel from './user-override-panel.vue'
import type { UserVO } from '@/types/models'
defineProps<{ user: UserVO; active: boolean }>()
defineEmits<{ busy: [value: boolean] }>()
</script>
<template>
    <Panel kind="authority" :user="user" :active="active" @busy="$emit('busy', $event)" />
</template>
