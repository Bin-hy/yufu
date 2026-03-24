<script lang="ts" setup>
// 系统介绍页面，无复杂逻辑，仅展示结构化信息
import UiButton from '@/components/ui/button/button.vue'
</script>

<template>
  <div class="space-y-8 font-inter">
    <!-- 标题与简介 -->
    <section class="space-y-2">
      <h1 class="text-2xl md:text-3xl font-bold text-gray-800">系统介绍</h1>
      <p class="text-gray-600">Cobotrix 是一个面向机器人与自动化场景的全栈解决方案，提供统一的设备接入、权限控制、数据记录与可视化管理能力。</p>
    </section>

    <!-- 核心特性 -->
    <section>
      <h2 class="text-lg md:text-xl font-semibold text-gray-800 mb-4">核心特性</h2>
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        <div class="bg-card text-card-foreground rounded-lg shadow-sm p-4">
          <h3 class="text-base font-semibold text-gray-800">统一设备接入</h3>
          <p class="text-gray-600 text-sm mt-1">支持多类型设备的注册与心跳，统一管理连接与状态。</p>
        </div>
        <div class="bg-card text-card-foreground rounded-lg shadow-sm p-4">
          <h3 class="text-base font-semibold text-gray-800">RBAC 权限</h3>
          <p class="text-gray-600 text-sm mt-1">基于角色的访问控制，保证操作边界清晰与安全。</p>
        </div>
        <div class="bg-card text-card-foreground rounded-lg shadow-sm p-4">
          <h3 class="text-base font-semibold text-gray-800">数据记录</h3>
          <p class="text-gray-600 text-sm mt-1">支持操作与事件记录，便于审计与问题追踪。</p>
        </div>
        <div class="bg-card text-card-foreground rounded-lg shadow-sm p-4">
          <h3 class="text-base font-semibold text-gray-800">可观测性</h3>
          <p class="text-gray-600 text-sm mt-1">提供基础监控与统计视图，快速把握系统运行状况。</p>
        </div>
        <div class="bg-card text-card-foreground rounded-lg shadow-sm p-4">
          <h3 class="text-base font-semibold text-gray-800">开放接口</h3>
          <p class="text-gray-600 text-sm mt-1">RESTful API，内置 Swagger 文档，便于集成拓展。</p>
        </div>
        <div class="bg-card text-card-foreground rounded-lg shadow-sm p-4">
          <h3 class="text-base font-semibold text-gray-800">前后端解耦</h3>
          <p class="text-gray-600 text-sm mt-1">前端 `cobotweb` 与后端 `cobotsrv`/`cobotlet` 分离，独立发布。</p>
        </div>
      </div>
    </section>

    <!-- 架构概览 -->
    <section>
      <h2 class="text-lg md:text-xl font-semibold text-gray-800 mb-4">架构概览</h2>
      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div class="bg-card text-card-foreground rounded-lg shadow-sm p-4">
          <h3 class="text-base font-semibold text-gray-800">Web 前端（cobotweb）</h3>
        <p class="text-gray-600 text-sm mt-1">基于 Vue3 + Vite + Pinia + Tailwind/Shadcn UI，提供管理与可视化界面。</p>
        </div>
        <div class="bg-card text-card-foreground rounded-lg shadow-sm p-4">
          <h3 class="text-base font-semibold text-gray-800">服务端（cobotsrv）</h3>
          <p class="text-gray-600 text-sm mt-1">Rust 编写，提供认证、权限、数据接口与记录中间件。</p>
        </div>
        <div class="bg-card text-card-foreground rounded-lg shadow-sm p-4">
          <h3 class="text-base font-semibold text-gray-800">边缘组件（cobotlet）</h3>
          <p class="text-gray-600 text-sm mt-1">面向设备侧的轻量服务，负责数据采集与任务执行。</p>
        </div>
      </div>
    </section>

    <!-- 快速上手 -->
    <section>
      <h2 class="text-lg md:text-xl font-semibold text-gray-800 mb-4">快速上手</h2>
      <ol class="list-decimal list-inside space-y-2 text-gray-700">
        <li>准备后端服务 `cobotsrv` 并配置数据库与认证。</li>
        <li>部署前端 `cobotweb`，配置 `.env` 指向后端地址。</li>
        <li>根据角色分配权限，创建用户并登录系统。</li>
        <li>接入设备或模块，开启记录与监控视图。</li>
      </ol>
    </section>

    <!-- 相关入口 -->
    <section>
      <h2 class="text-lg md:text-xl font-semibold text-gray-800 mb-4">更多入口</h2>
      <div class="flex flex-wrap gap-2">
        <router-link to="/users"><UiButton variant="outline">用户管理</UiButton></router-link>
        <router-link to="/authority"><UiButton variant="outline">权限管理</UiButton></router-link>
        <router-link to="/dashboard"><UiButton>返回仪表盘</UiButton></router-link>
      </div>
    </section>
  </div>
</template>