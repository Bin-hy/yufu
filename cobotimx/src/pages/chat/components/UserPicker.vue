<script setup lang="ts">
import { ref, computed } from "vue";
import { useNicknames } from "@/composables/useNicknames";

const props = defineProps<{
  multiple?: boolean;
  modelValue: number[];
}>();
const emit = defineEmits<{ (e: "update:modelValue", val: number[]): void }>();

const { users } = useNicknames();
const search = ref("");

const filtered = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return users.value;
  return users.value.filter(
    (u) =>
      u.username.toLowerCase().includes(q) ||
      (u.nickname || "").toLowerCase().includes(q),
  );
});

function toggle(id: number) {
  if (props.multiple) {
    const sel = [...props.modelValue];
    const idx = sel.indexOf(id);
    if (idx >= 0) sel.splice(idx, 1);
    else sel.push(id);
    emit("update:modelValue", sel);
  } else {
    emit("update:modelValue", props.modelValue[0] === id ? [] : [id]);
  }
}

function isSelected(id: number) {
  return props.modelValue.includes(id);
}
</script>

<template>
  <div class="border border-gray-200 rounded-lg bg-white max-h-60 flex flex-col">
    <div class="px-2 py-1.5 border-b border-gray-100">
      <input
        v-model="search"
        type="text"
        class="w-full px-2 py-1 text-sm border border-gray-200 rounded focus:outline-none focus:ring-1 focus:ring-blue-500"
        placeholder="搜索用户名或昵称..."
      />
    </div>
    <ul class="overflow-y-auto flex-1">
      <li
        v-for="u in filtered"
        :key="u.id"
        @click="toggle(u.id)"
        :class="[
          'flex items-center gap-2 px-3 py-1.5 text-sm cursor-pointer hover:bg-blue-50 transition-colors',
          isSelected(u.id) ? 'bg-blue-100' : '',
        ]"
      >
        <span
          :class="[
            'w-4 h-4 rounded border flex items-center justify-center text-xs flex-shrink-0',
            isSelected(u.id) ? 'bg-blue-600 border-blue-600 text-white' : 'border-gray-300',
          ]"
        >
          {{ isSelected(u.id) ? '✓' : '' }}
        </span>
        <span class="font-medium truncate">{{ u.nickname || u.username }}</span>
        <span v-if="u.nickname" class="text-gray-400 text-xs truncate">@{{ u.username }}</span>
      </li>
      <li v-if="filtered.length === 0" class="px-3 py-4 text-center text-gray-400 text-sm">
        无匹配用户
      </li>
    </ul>
  </div>
</template>
