<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useChatStore } from "@/stores/chat";
import { useNicknames } from "@/composables/useNicknames";
import { conversationApi } from "@/api/conversations";
import { groupApi } from "@/api/groups";
import UserPicker from "./UserPicker.vue";

const emit = defineEmits<{ (e: "close"): void }>();
const chat = useChatStore();
const { get: getNick } = useNicknames();

const memberIds = ref<number[]>([]);
const addSelected = ref<number[]>([]);
const addError = ref("");
const loading = ref(false);

onMounted(async () => {
  if (!chat.activeConversationId) return;
  try {
    const res = await conversationApi.members(chat.activeConversationId);
    if (res.data.data) {
      memberIds.value = res.data.data.member_ids;
    }
  } catch { /* ignore */ }
});

async function handleAdd() {
  if (!chat.activeConversationId) return;
  if (addSelected.value.length === 0) {
    addError.value = "请至少选择一个用户";
    return;
  }
  loading.value = true;
  addError.value = "";
  try {
    const res = await groupApi.addMembers(chat.activeConversationId, addSelected.value);
    if (res.data.data) {
      const existing = new Set(memberIds.value);
      for (const id of res.data.data.added_member_ids) {
        existing.add(id);
      }
      memberIds.value = Array.from(existing);
      addSelected.value = [];
      if (res.data.data.skipped_member_ids.length > 0) {
        addError.value = `已跳过：${res.data.data.skipped_member_ids.join(", ")}`;
      }
    }
  } catch (e: unknown) {
    const msg = (e as { response?: { data?: { message?: string } } })?.response?.data?.message;
    addError.value = msg || "添加成员失败";
  } finally {
    loading.value = false;
  }
}

async function handleRemove(userId: number) {
  if (!chat.activeConversationId) return;
  const name = getNick(userId);
  if (!confirm(`确定移除 ${name}？`)) return;
  try {
    await groupApi.removeMember(chat.activeConversationId, userId);
    memberIds.value = memberIds.value.filter((id) => id !== userId);
  } catch (e: unknown) {
    const msg = (e as { response?: { data?: { message?: string } } })?.response?.data?.message;
    alert(msg || "移除成员失败");
  }
}

async function handleTransfer(userId: number) {
  if (!chat.activeConversationId) return;
  const name = getNick(userId);
  if (!confirm(`确定将群主转让给 ${name}？`)) return;
  try {
    await groupApi.transferOwner(chat.activeConversationId, userId);
    alert("群主已转让");
  } catch (e: unknown) {
    const msg = (e as { response?: { data?: { message?: string } } })?.response?.data?.message;
    alert(msg || "转让失败");
  }
}

async function handleLeave() {
  if (!chat.activeConversationId) return;
  if (!confirm("确定退出此群聊？")) return;
  try {
    await groupApi.leave(chat.activeConversationId);
    chat.activeConversationId = null;
    chat.clearMessages();
    await chat.loadConversations();
    emit("close");
  } catch (e: unknown) {
    const msg = (e as { response?: { data?: { message?: string } } })?.response?.data?.message;
    alert(msg || "退群失败");
  }
}
</script>

<template>
  <div class="absolute inset-0 z-10 flex">
    <!-- Backdrop -->
    <div class="flex-1 bg-black/30" @click="emit('close')" />

    <!-- Panel -->
    <div class="w-80 bg-white border-l border-gray-200 flex flex-col overflow-y-auto shadow-lg">
      <div class="flex items-center justify-between px-4 py-3 border-b border-gray-200">
        <h3 class="font-semibold">群设置</h3>
        <button @click="emit('close')" class="text-gray-400 hover:text-gray-600 text-lg">&times;</button>
      </div>

      <div class="p-4 space-y-4">
        <!-- Members -->
        <div>
          <h4 class="text-sm font-medium text-gray-600 mb-2">
            成员 ({{ memberIds.length }})
          </h4>
          <ul class="space-y-1 max-h-60 overflow-y-auto">
            <li
              v-for="id in memberIds"
              :key="id"
              class="flex items-center justify-between text-sm py-1 px-2 rounded hover:bg-gray-50"
            >
              <span>{{ getNick(id) }}</span>
              <div class="flex gap-1">
                <button
                  @click="handleTransfer(id)"
                  class="text-xs text-blue-600 hover:underline"
                  title="转让群主"
                >
                  转让
                </button>
                <button
                  @click="handleRemove(id)"
                  class="text-xs text-red-600 hover:underline"
                  title="移除成员"
                >
                  移除
                </button>
              </div>
            </li>
          </ul>
        </div>

        <!-- Add members -->
        <div>
          <h4 class="text-sm font-medium text-gray-600 mb-2">添加成员</h4>
          <UserPicker v-model="addSelected" multiple />
          <button
            @click="handleAdd"
            :disabled="loading || addSelected.length === 0"
            class="w-full mt-2 px-3 py-1.5 bg-green-600 text-white rounded text-sm hover:bg-green-700 disabled:opacity-50 transition-colors"
          >
            添加
          </button>
          <p v-if="addError" class="text-xs text-red-500 mt-1">{{ addError }}</p>
        </div>

        <!-- Leave -->
        <div>
          <button
            @click="handleLeave"
            class="w-full py-2 border border-red-300 text-red-600 rounded-lg text-sm hover:bg-red-50 transition-colors"
          >
            退出群聊
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
