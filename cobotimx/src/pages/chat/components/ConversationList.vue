<script setup lang="ts">
import { ref } from "vue";
import { useChatStore } from "@/stores/chat";
import { useAuthStore } from "@/stores/auth";
import { conversationApi } from "@/api/conversations";
import type { CreateConversationRequest } from "@/types/models";
import UserPicker from "./UserPicker.vue";

const chat = useChatStore();
const auth = useAuthStore();

const showCreate = ref(false);
const createType = ref<1 | 2>(1);
const createTitle = ref("");
const selectedMembers = ref<number[]>([]);
const createError = ref("");

function selectConversation(id: number) {
  chat.activeConversationId = id;
  chat.clearMessages();
  chat.loadMessages(id);
  chat.loadActiveConversationDetail();
  conversationApi.clearUnread(id).then(() => {
    const c = chat.conversations.find((c) => c.conversation_id === id);
    if (c) c.unread_count = 0;
  });
}

async function handleCreate() {
  createError.value = "";
  if (selectedMembers.value.length === 0) {
    createError.value = "请至少选择一个成员";
    return;
  }
  const body: CreateConversationRequest = {
    conversation_type: createType.value,
    member_ids: selectedMembers.value,
  };
  if (createType.value === 2) {
    body.title = createTitle.value.trim() || "群聊";
  }
  try {
    const res = await conversationApi.create(body);
    if (res.data.data) {
      showCreate.value = false;
      selectedMembers.value = [];
      await chat.loadConversations();
      selectConversation(res.data.data.conversation_id);
    }
  } catch (e: unknown) {
    const msg = (e as { response?: { data?: { message?: string } } })?.response?.data?.message;
    createError.value = msg || "创建会话失败";
  }
}

async function handleLogout() {
  await auth.logout();
}

function convIcon(type: 1 | 2): string {
  return type === 1 ? "👤" : "👥";
}
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- Header -->
    <div class="flex items-center justify-between px-4 py-3 border-b border-gray-200">
      <span class="font-semibold text-lg">消息</span>
      <div class="flex items-center gap-2">
        <button
          @click="showCreate = !showCreate"
          class="text-sm px-3 py-1 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
        >
          +
        </button>
        <button
          @click="handleLogout"
          class="text-sm px-3 py-1 text-gray-600 hover:text-red-600 transition-colors"
          title="退出登录"
        >
          &larr;
        </button>
      </div>
    </div>

    <!-- Create form -->
    <div v-if="showCreate" class="px-4 py-3 border-b border-gray-200 bg-gray-50 space-y-2">
      <div class="flex gap-2">
        <button
          @click="createType = 1"
          :class="createType === 1 ? 'bg-blue-600 text-white' : 'bg-gray-200 text-gray-700'"
          class="px-3 py-1 rounded text-sm transition-colors"
        >
          单聊
        </button>
        <button
          @click="createType = 2"
          :class="createType === 2 ? 'bg-blue-600 text-white' : 'bg-gray-200 text-gray-700'"
          class="px-3 py-1 rounded text-sm transition-colors"
        >
          群聊
        </button>
      </div>
      <input
        v-if="createType === 2"
        v-model="createTitle"
        type="text"
        placeholder="群组名称"
        class="w-full px-2 py-1 border border-gray-300 rounded text-sm"
      />
      <UserPicker
        v-model="selectedMembers"
        :multiple="createType === 2"
      />
      <p v-if="createError" class="text-red-500 text-xs">{{ createError }}</p>
      <div class="flex gap-2">
        <button
          @click="handleCreate"
          class="px-3 py-1 bg-green-600 text-white rounded text-sm hover:bg-green-700 transition-colors"
        >
          创建
        </button>
        <button
          @click="showCreate = false"
          class="px-3 py-1 bg-gray-300 text-gray-700 rounded text-sm hover:bg-gray-400 transition-colors"
        >
          取消
        </button>
      </div>
    </div>

    <!-- Search -->
    <div class="px-4 py-2">
      <input
        type="text"
        placeholder="搜索会话..."
        class="w-full px-3 py-1.5 bg-gray-100 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
      />
    </div>

    <!-- List -->
    <div class="flex-1 overflow-y-auto">
      <div
        v-for="c in chat.conversations"
        :key="c.conversation_id"
        @click="selectConversation(c.conversation_id)"
        :class="[
          'flex items-center gap-3 px-4 py-3 cursor-pointer hover:bg-gray-50 transition-colors border-b border-gray-100',
          chat.activeConversationId === c.conversation_id ? 'bg-blue-50' : '',
        ]"
      >
        <span class="text-2xl">{{ convIcon(c.conversation_type) }}</span>
        <div class="flex-1 min-w-0">
          <div class="flex items-center justify-between">
            <span class="font-medium text-sm truncate">
              {{ c.title || `会话 #${c.conversation_id}` }}
            </span>
            <span
              v-if="c.unread_count > 0"
              class="ml-2 px-2 py-0.5 bg-red-500 text-white text-xs rounded-full"
            >
              {{ c.unread_count > 99 ? "99+" : c.unread_count }}
            </span>
          </div>
          <p class="text-xs text-gray-500 truncate mt-0.5">
            {{ c.last_message_preview || "暂无消息" }}
          </p>
        </div>
      </div>
      <div
        v-if="chat.conversations.length === 0"
        class="text-center text-gray-400 text-sm py-12"
      >
        暂无会话
      </div>
    </div>

    <!-- Footer -->
    <div class="px-4 py-2 border-t border-gray-200 text-xs text-gray-500">
      当前登录：{{ auth.nickname || auth.username }}
    </div>
  </div>
</template>
