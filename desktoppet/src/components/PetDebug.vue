<script setup>
import { ref, watch, onMounted, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { usePanelDrag } from '../composables/usePanelDrag'

const props = defineProps({
  visible: { type: Boolean, default: false },
})
const emit = defineEmits(['close'])

const panelRef = ref(null)
const headerRef = ref(null)

const states = ref([])
const selected = ref('')
const errMsg = ref('')

let { attach: attachDrag, detach: detachDrag } = usePanelDrag(panelRef, headerRef)

async function loadStates() {
  try {
    const list = await invoke('get_all_states')
    states.value = list || []
    if (states.value.length && !selected.value) {
      selected.value = states.value[0].id
    }
  } catch (e) {
    errMsg.value = `加载状态列表失败: ${e}`
  }
}

async function applyForce() {
  errMsg.value = ''
  try {
    await invoke('force_state', { target: selected.value })
  } catch (e) {
    errMsg.value = `切换失败: ${e}`
  }
}

async function resumeAuto() {
  errMsg.value = ''
  try {
    await invoke('resume_auto_state')
  } catch (e) {
    errMsg.value = `恢复失败: ${e}`
  }
}

watch(() => props.visible, (val) => {
  if (val) {
    loadStates()
    // 偏移定位：如果设置也在右侧，错开显示
    if (panelRef.value) {
      panelRef.value.style.right = '8px'
      panelRef.value.style.top = '8px'
      panelRef.value.style.left = 'auto'
      panelRef.value.style.bottom = 'auto'
    }
    nextTick(() => attachDrag())
  } else {
    detachDrag()
  }
})

onMounted(() => {
  if (props.visible) {
    nextTick(() => attachDrag())
  }
})
</script>

<template>
  <transition name="panel">
    <div v-if="visible" ref="panelRef" class="debug-panel">
      <div ref="headerRef" class="debug-header">
        <div class="header-left">
          <span class="header-dot"></span>
          <h3>状态切换</h3>
        </div>
        <button class="close-btn" @click="emit('close')">
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M1 1l12 12M13 1L1 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
        </button>
      </div>

      <div class="debug-body">
        <div class="form-group">
          <label>选择状态</label>
          <select v-model="selected">
            <option v-for="s in states" :key="s.id" :value="s.id">{{ s.label }}</option>
          </select>
        </div>

        <div class="state-desc" v-if="selected">
          {{ states.find((s) => s.id === selected)?.description || '无描述' }}
        </div>

        <div class="actions">
          <button class="primary-btn" @click="applyForce">强制切换</button>
          <button class="ghost-btn" @click="resumeAuto">恢复自动</button>
        </div>

        <div v-if="errMsg" class="err-msg">{{ errMsg }}</div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.debug-panel {
  position: fixed;
  top: 8px;
  right: 8px;
  width: 300px;
  background: var(--color-surface);
  backdrop-filter: blur(24px) saturate(1.4);
  -webkit-backdrop-filter: blur(24px) saturate(1.4);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-panel);
  overflow: hidden;
  z-index: 50;
  pointer-events: auto;
  border: 1px solid var(--color-border);
}

.debug-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  flex-shrink: 0;
  cursor: grab;
  border-bottom: 1px solid var(--color-border);
}
.debug-header:active {
  cursor: grabbing;
}
.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
}
.header-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--color-primary);
  box-shadow: 0 0 6px rgba(99, 102, 241, 0.4);
}
.debug-header h3 {
  margin: 0;
  font-size: var(--font-size-md);
  font-weight: 600;
  color: var(--color-text);
}

.close-btn {
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all var(--transition-fast);
}
.close-btn:hover {
  background: var(--color-danger-soft);
  color: var(--color-danger);
}

.debug-body {
  padding: 16px;
}

.form-group {
  display: flex;
  flex-direction: column;
  margin-bottom: 12px;
}
.form-group label {
  font-size: var(--font-size-sm);
  font-weight: 500;
  color: var(--color-text-secondary);
  margin-bottom: 6px;
  text-transform: uppercase;
  letter-spacing: 0.3px;
}
.form-group select {
  width: 100%;
  padding: 8px 12px;
  font-size: var(--font-size-base);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-input-bg);
  color: var(--color-text);
  outline: none;
  cursor: pointer;
  transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg width='10' height='6' viewBox='0 0 10 6' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1l4 4 4-4' stroke='%2394a3b8' stroke-width='1.5' stroke-linecap='round'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 10px center;
  padding-right: 30px;
}
.form-group select:focus {
  border-color: var(--color-primary-light);
  box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.1);
}

.state-desc {
  margin: 2px 0 16px;
  padding: 10px 12px;
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  background: var(--color-input-bg);
  border-radius: var(--radius-md);
  line-height: 1.5;
}

.actions {
  display: flex;
  gap: 10px;
}

.primary-btn,
.ghost-btn {
  flex: 1;
  padding: 9px 14px;
  font-size: var(--font-size-base);
  font-weight: 500;
  border-radius: var(--radius-md);
  cursor: pointer;
  border: none;
  transition: all var(--transition-fast);
}

.primary-btn {
  background: var(--color-primary);
  color: #fff;
}
.primary-btn:hover {
  background: var(--color-primary-light);
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(99, 102, 241, 0.3);
}

.ghost-btn {
  background: var(--color-input-bg);
  color: var(--color-text-secondary);
  border: 1px solid var(--color-border);
}
.ghost-btn:hover {
  background: var(--color-primary-soft);
  color: var(--color-primary);
  border-color: var(--color-primary-light);
}

.err-msg {
  margin-top: 12px;
  padding: 8px 12px;
  background: var(--color-danger-soft);
  color: var(--color-danger);
  font-size: var(--font-size-sm);
  border-radius: var(--radius-md);
}

/* transitions */
.panel-enter-active {
  transition: opacity 0.2s ease, transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
}
.panel-leave-active {
  transition: opacity 0.15s ease, transform 0.2s ease;
}
.panel-enter-from {
  opacity: 0;
  transform: scale(0.92) translateY(12px);
}
.panel-leave-to {
  opacity: 0;
  transform: scale(0.92) translateY(8px);
}
</style>
