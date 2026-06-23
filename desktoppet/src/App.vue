<script setup>
import Pet from './components/Pet.vue'
</script>

<template>
  <div class="app-root">
    <Pet />
  </div>
</template>

<style>
/* === 设计令牌 === */
:root {
  --color-primary: #6366f1;
  --color-primary-light: #818cf8;
  --color-primary-soft: rgba(99, 102, 241, 0.08);
  --color-danger: #ef4444;
  --color-danger-soft: rgba(239, 68, 68, 0.08);
  --color-success: #10b981;
  --color-text: #1e293b;
  --color-text-secondary: #64748b;
  --color-text-muted: #94a3b8;
  --color-surface: rgba(255, 255, 255, 0.82);
  --color-surface-hover: rgba(255, 255, 255, 0.95);
  --color-border: rgba(226, 232, 240, 0.8);
  --color-input-bg: rgba(241, 245, 249, 0.7);
  --radius-sm: 6px;
  --radius-md: 10px;
  --radius-lg: 16px;
  --radius-xl: 20px;
  --shadow-panel: 0 1px 3px rgba(0,0,0,0.04), 0 8px 32px rgba(0,0,0,0.12);
  --shadow-hover: 0 2px 6px rgba(0,0,0,0.06), 0 12px 40px rgba(0,0,0,0.16);
  --font-size-xs: 10px;
  --font-size-sm: 11px;
  --font-size-base: 13px;
  --font-size-md: 14px;
  --transition-fast: 0.15s ease;
  --transition-normal: 0.25s ease;
}

/* 全局样式：透明背景 */
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body, #app {
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: transparent;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', sans-serif;
  color: var(--color-text);
  font-size: var(--font-size-base);
  -webkit-font-smoothing: antialiased;
}

.app-root {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
}
</style>
