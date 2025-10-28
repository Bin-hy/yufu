<script lang="ts" setup>
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useUserStore } from '@/pinia/stores/user'
import UiButton from '@/components/ui/button/button.vue'

const router = useRouter()
const userStore = useUserStore()

const goHome = () => {
  router.push('/')
}

const goDashboard = () => {
  router.push('/dashboard')
}

const handleLogout = () => {
  // 清除用户信息
  userStore.clearUserInfo()
  // 重定向到登录页面
  router.push('/')
}

// 添加动画效果
onMounted(() => {
  // 为数字添加动画
  const numbers = document.querySelectorAll('.error-number span')
  numbers.forEach((num, index) => {
    setTimeout(() => {
      (num as HTMLElement).classList.add('opacity-100', 'translate-y-0')
    }, index * 150)
  })
})
</script>

<template>
  <div class="min-h-screen flex flex-col items-center justify-center bg-muted py-16 px-4 sm:px-6 lg:px-8">
    <!-- 装饰背景元素 -->
    <div class="absolute top-0 left-0 w-full h-full overflow-hidden pointer-events-none">
      <div class="absolute -top-40 -right-40 w-96 h-96 bg-secondary/5 rounded-full"></div>
      <div class="absolute -bottom-40 -left-40 w-96 h-96 bg-red-500/5 rounded-full"></div>
    </div>
    
    <!-- 主内容区域 -->
    <div class="relative max-w-3xl w-full text-center">
      <!-- 错误数字 -->
      <div class="flex items-baseline justify-center mb-6">
        <div class="error-number inline-block">
          <span class="text-[12rem] font-bold text-secondary/90 tracking-tight block opacity-0 translate-y-10 transition-all duration-700">4</span>
          <span class="text-[12rem] font-bold text-red-500/90 tracking-tight block opacity-0 translate-y-10 transition-all duration-700">0</span>
          <span class="text-[12rem] font-bold text-secondary/90 tracking-tight block opacity-0 translate-y-10 transition-all duration-700">3</span>
        </div>
      </div>
      
      <!-- 错误标题和描述 -->
      <h2 class="text-3xl font-bold text-gray-900 mb-4">权限不足</h2>
      <p class="text-gray-600 mb-8 max-w-2xl mx-auto text-lg">
        抱歉，您没有权限访问此页面。
        请联系管理员获取适当的权限，或者尝试其他操作。
      </p>
      
      <!-- 导航按钮 -->
      <div class="flex flex-col sm:flex-row justify-center space-y-4 sm:space-y-0 sm:space-x-4">
        <UiButton size="lg" @click="goDashboard">回到仪表盘</UiButton>
        <UiButton size="lg" variant="outline" @click="handleLogout">切换账号</UiButton>
        <UiButton size="lg" variant="secondary" @click="goHome">首页</UiButton>
      </div>
      
      <!-- 帮助信息卡片 -->
      <div class="mt-12 p-6 bg-card text-card-foreground rounded-lg shadow-sm inline-block w-full max-w-md">
        <h3 class="text-lg font-semibold text-foreground mb-3">需要帮助？</h3>
        <p class="text-sm text-muted-foreground mb-4">
          如需访问此页面，请联系系统管理员申请相应权限。
        </p>
        <div class="flex items-center text-sm text-muted-foreground">
          <svg class="h-5 w-5 mr-2 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"></path>
          </svg>
          <span>联系管理员: admin@cobotrix.com</span>
        </div>
      </div>
    </div>
    
    <!-- 页脚信息 -->
    <div class="mt-16 text-center text-sm text-gray-500">
      <p>&copy; {{ new Date().getFullYear() }} Cobotrix 管理系统</p>
    </div>
  </div>
</template>

<style scoped>
/* 自定义动画 */
@keyframes float {
  0%, 100% {
    transform: translateY(0);
  }
  50% {
    transform: translateY(-10px);
  }
}

.error-number span {
  display: inline-block;
  animation: float 3s ease-in-out infinite;
}

.error-number span:nth-child(1) {
  animation-delay: 0s;
}

.error-number span:nth-child(2) {
  animation-delay: 0.3s;
}

.error-number span:nth-child(3) {
  animation-delay: 0.6s;
}

/* 响应式调整 */
@media (max-width: 640px) {
  .error-number span {
    font-size: 8rem !important;
  }
  
  h2 {
    font-size: 2rem !important;
  }
}
</style>
