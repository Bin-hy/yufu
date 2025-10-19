<script lang="ts" setup>
import { useRouter } from 'vue-router'
import { useUserStore } from '@/pinia/stores/user'
import {computed, onMounted, reactive} from 'vue'
import { userDataApi } from '@/pages/dashboard/api'
const router = useRouter()
const userStore = useUserStore()

// 获取用户名（如果已登录）
const username = computed(() => {

  return userStore.userInfo?.username || 'Guest'
})

// 退出登录
const handleLogout = () => {
  // 清除用户信息
  userStore.clearUserInfo()
  
  // 重定向到登录页面
  router.push('/')
}

async function load_data(){
  // 加载用户信息
  // let user_data = await userDataApi()
  // user_data.data
  // userStore.setUserInfo()
}

onMounted(()=>{
  load_data()
})
</script>

<template>
  <div class="min-h-screen bg-gray-50">
    <!-- 顶部导航栏 -->
    <header class="bg-white shadow">
      <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div class="flex justify-between h-16">
          <div class="flex items-center">
            <h1 class="text-xl font-semibold text-gray-800">管理界面</h1>
          </div>
          <div class="flex items-center space-x-4">
            <span class="text-sm text-gray-600">欢迎，{{ username }}</span>
            <button
              @click="handleLogout"
              class="px-3 py-1 text-sm font-medium text-white bg-red-600 rounded-md hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-red-500 transition duration-200 ease-in-out"
            >
              退出登录
            </button>
          </div>
        </div>
      </div>
    </header>

    <!-- 主内容区域 -->
    <main class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
      <!-- 欢迎信息 -->
      <div class="mb-8">
        <h2 class="text-2xl font-bold text-gray-800">欢迎来到管理控制台</h2>
        <p class="mt-2 text-gray-600">这是您的个人仪表盘，您可以在这里管理您的账户和查看相关信息。</p>
      </div>

      <!-- 功能卡片区域 -->
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        <!-- 功能卡片 1 -->
        <div class="bg-white overflow-hidden shadow rounded-lg hover:shadow-md transition-shadow duration-200">
          <div class="p-6">
            <div class="flex items-center">
              <div class="flex-shrink-0 bg-blue-500 rounded-md p-3">
                <svg class="h-6 w-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"></path>
                </svg>
              </div>
              <div class="ml-5 w-0 flex-1">
                <dl>
                  <dt class="text-sm font-medium text-gray-500 truncate">账户概览</dt>
                  <dd class="mt-1 text-lg font-semibold text-gray-900">查看和管理您的个人信息</dd>
                </dl>
              </div>
            </div>
          </div>
        </div>

        <!-- 功能卡片 2 -->
        <div class="bg-white overflow-hidden shadow rounded-lg hover:shadow-md transition-shadow duration-200">
          <div class="p-6">
            <div class="flex items-center">
              <div class="flex-shrink-0 bg-green-500 rounded-md p-3">
                <svg class="h-6 w-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 3.055A9.001 9.001 0 1020.945 13H11V3.055z"></path>
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20.488 9H15V3.512A9.025 9.025 0 0120.488 9z"></path>
                </svg>
              </div>
              <div class="ml-5 w-0 flex-1">
                <dl>
                  <dt class="text-sm font-medium text-gray-500 truncate">数据分析</dt>
                  <dd class="mt-1 text-lg font-semibold text-gray-900">查看您的使用统计和数据报告</dd>
                </dl>
              </div>
            </div>
          </div>
        </div>

        <!-- 功能卡片 3 -->
        <div class="bg-white overflow-hidden shadow rounded-lg hover:shadow-md transition-shadow duration-200">
          <div class="p-6">
            <div class="flex items-center">
              <div class="flex-shrink-0 bg-purple-500 rounded-md p-3">
                <svg class="h-6 w-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"></path>
                </svg>
              </div>
              <div class="ml-5 w-0 flex-1">
                <dl>
                  <dt class="text-sm font-medium text-gray-500 truncate">用户管理</dt>
                  <dd class="mt-1 text-lg font-semibold text-gray-900">管理您的个人设置和偏好</dd>
                </dl>
              </div>
            </div>
          </div>
        </div>
      </div>
    </main>
  </div>
</template>
