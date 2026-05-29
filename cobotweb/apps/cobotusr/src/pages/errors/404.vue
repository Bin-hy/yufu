<script lang="ts" setup>
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'

const router = useRouter()

const goHome = () => {
  router.push('/')
}

const goDashboard = () => {
  router.push('/dashboard')
}

// 添加一些动画效果
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
      <div class="absolute -top-40 -right-40 w-96 h-96 bg-primary/5 rounded-full"></div>
      <div class="absolute -bottom-40 -left-40 w-96 h-96 bg-secondary/5 rounded-full"></div>
    </div>
    
    <!-- 主内容区域 -->
    <div class="relative max-w-3xl w-full text-center">
      <!-- 错误数字 -->
      <div class="flex items-baseline justify-center mb-6">
        <div class="error-number inline-block">
          <span class="text-[12rem] font-bold text-primary/90 tracking-tight block opacity-0 translate-y-10 transition-all duration-700">4</span>
          <span class="text-[12rem] font-bold text-secondary/90 tracking-tight block opacity-0 translate-y-10 transition-all duration-700">0</span>
          <span class="text-[12rem] font-bold text-primary/90 tracking-tight block opacity-0 translate-y-10 transition-all duration-700">4</span>
        </div>
      </div>
      
      <!-- 错误标题和描述 -->
      <h2 class="text-3xl font-bold text-gray-900 mb-4">抱歉，您访问的页面不存在</h2>
      <p class="text-gray-600 mb-8 max-w-2xl mx-auto text-lg">
        您可能输入了错误的地址，或者该页面已被移动或删除。
        请检查URL拼写，或返回首页继续浏览。
      </p>
      
      <!-- 导航按钮 -->
      <div class="flex flex-col sm:flex-row justify-center space-y-4 sm:space-y-0 sm:space-x-4">
        <UiButton size="lg" @click="goHome">返回首页</UiButton>
        <UiButton size="lg" variant="outline" @click="goDashboard">前往仪表盘</UiButton>
      </div>
      
      <!-- 搜索提示 -->
      <div class="mt-12 p-4 bg-card text-card-foreground rounded-lg shadow-sm inline-block">
        <p class="text-sm text-gray-500">
          您也可以尝试搜索您想要的内容
        </p>
        <div class="relative mt-2">
          <UiInput 
            type="text" 
            placeholder="搜索..." 
            class="pl-10 w-64"
          />
          <svg class="absolute left-3 top-2.5 h-5 w-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"></path>
          </svg>
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
