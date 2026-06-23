<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'

const props = defineProps({
  visible: { type: Boolean, default: false },
  x: { type: Number, default: 0 },
  y: { type: Number, default: 0 },
})

const emit = defineEmits(['close', 'select'])

const menuRef = ref(null)

const items = [
  { id: 'demo', label: '状态切换', color: '#6366f1' },
  { id: 'settings', label: '设置', color: '#8b5cf6' },
  { id: 'separator', label: '', separator: true },
  { id: 'quit', label: '退出', color: '#ef4444', danger: true },
]

function handleSelect(id) {
  emit('select', id)
  emit('close')
}

function handleClickOutside(e) {
  if (menuRef.value && !menuRef.value.contains(e.target)) {
    emit('close')
  }
}

const adjustedStyle = computed(() => {
  if (!props.visible) return {}
  const menuW = 150
  const menuH = 150
  const winW = window.innerWidth
  const winH = window.innerHeight
  let left = props.x
  let top = props.y
  if (left + menuW > winW) left = winW - menuW - 4
  if (top + menuH > winH) top = winH - menuH - 4
  if (left < 4) left = 4
  if (top < 4) top = 4
  return {
    left: left + 'px',
    top: top + 'px',
  }
})

onMounted(() => {
  document.addEventListener('mousedown', handleClickOutside)
})
onUnmounted(() => {
  document.removeEventListener('mousedown', handleClickOutside)
})
</script>

<template>
  <transition name="menu">
    <div
      v-if="visible"
      ref="menuRef"
      class="pet-menu"
      :style="adjustedStyle"
      @contextmenu.prevent
    >
      <template v-for="item in items" :key="item.id">
        <div v-if="item.separator" class="menu-separator"></div>
        <div
          v-else
          class="menu-item"
          :class="{ danger: item.danger }"
          @click="handleSelect(item.id)"
        >
          <span class="menu-dot" :style="{ background: item.color }"></span>
          <span class="menu-label">{{ item.label }}</span>
        </div>
      </template>
    </div>
  </transition>
</template>

<style scoped>
.pet-menu {
  position: fixed;
  min-width: 150px;
  background: var(--color-surface);
  backdrop-filter: blur(24px) saturate(1.4);
  -webkit-backdrop-filter: blur(24px) saturate(1.4);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-panel);
  padding: 6px;
  z-index: 100;
  user-select: none;
  border: 1px solid var(--color-border);
}

.menu-item {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  font-size: var(--font-size-base);
  font-weight: 500;
  color: var(--color-text);
  cursor: pointer;
  border-radius: var(--radius-sm);
  transition: all 0.12s ease;
  position: relative;
}

.menu-item:hover {
  background: var(--color-primary-soft);
}

.menu-item.danger:hover {
  background: var(--color-danger-soft);
}

.menu-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  margin-right: 10px;
  flex-shrink: 0;
}

.menu-label {
  flex: 1;
}

.menu-separator {
  height: 1px;
  background: var(--color-border);
  margin: 4px 8px;
}

.menu-enter-active,
.menu-leave-active {
  transition: opacity 0.15s ease, transform 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
}
.menu-enter-from,
.menu-leave-to {
  opacity: 0;
  transform: translateY(-6px) scale(0.95);
}
</style>
