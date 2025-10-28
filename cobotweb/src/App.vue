<script setup lang="ts">
// 全局错误提示宿主
import ToastHost from '@/components/Feedback/toast-host.vue'
</script>

<template>
    <div>
        <router-view/>
        <ToastHost />
    </div>

</template>
