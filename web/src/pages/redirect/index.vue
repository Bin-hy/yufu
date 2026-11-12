<script setup lang="ts">
import { useRoute, useRouter } from 'vue-router'
const route = useRoute(),
    router = useRouter()
const requested =
    typeof route.params.path === 'string'
        ? '/' + route.params.path
        : typeof route.query.redirect === 'string'
          ? route.query.redirect
          : '/dashboard'
const destination =
    requested.startsWith('/') &&
    !requested.startsWith('//') &&
    router.resolve(requested).meta.workspace
        ? requested
        : '/dashboard'
void router.replace(destination)
</script>
<template><p class="data-state" role="status">正在打开工作台…</p></template>
