<script setup>
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { listen } from '@tauri-apps/api/event'

const props = defineProps({
  anchorEl: { type: Object, default: null },
})

const visible = ref(false)
const text = ref('')
const anchorRect = ref({ top: 0, left: 0, width: 128, height: 128 })
let unlisten = null
let hideTimer = null
let posRafId = null

function updateAnchorRect() {
  if (props.anchorEl) {
    const r = props.anchorEl.getBoundingClientRect()
    anchorRect.value = { top: r.top, left: r.left, width: r.width, height: r.height }
  }
  posRafId = requestAnimationFrame(updateAnchorRect)
}

const bubbleStyle = computed(() => {
  const top = anchorRect.value.top - 10
  const centerX = anchorRect.value.left + anchorRect.value.width / 2
  return {
    left: `${centerX}px`,
    top: `${top}px`,
    transform: 'translate(-50%, -100%)',
  }
})

function showBubble(payload) {
  text.value = payload.text
  visible.value = true

  if (hideTimer) clearTimeout(hideTimer)
  hideTimer = setTimeout(() => {
    visible.value = false
  }, payload.duration_ms || 3000)
}

onMounted(async () => {
  unlisten = await listen('pet-bubble', (event) => {
    showBubble(event.payload)
  })
  updateAnchorRect()
})

onUnmounted(() => {
  if (unlisten) unlisten()
  if (hideTimer) clearTimeout(hideTimer)
  if (posRafId) cancelAnimationFrame(posRafId)
})
</script>

<template>
  <transition name="bubble">
    <div v-if="visible" class="pet-bubble" :style="bubbleStyle">
      <span class="bubble-text">{{ text }}</span>
      <span class="bubble-tail"></span>
    </div>
  </transition>
</template>

<style scoped>
.pet-bubble {
  position: fixed;
  max-width: 220px;
  padding: 8px 14px;
  background: var(--color-surface);
  backdrop-filter: blur(20px) saturate(1.4);
  -webkit-backdrop-filter: blur(20px) saturate(1.4);
  color: var(--color-text);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-panel);
  font-size: var(--font-size-base);
  font-weight: 500;
  line-height: 1.5;
  word-wrap: break-word;
  pointer-events: none;
  z-index: 10;
  white-space: normal;
  border: 1px solid var(--color-border);
}

.bubble-text {
  display: block;
  text-align: center;
}

.bubble-tail {
  position: absolute;
  bottom: -5px;
  left: 50%;
  transform: translateX(-50%);
  width: 0;
  height: 0;
  border-left: 6px solid transparent;
  border-right: 6px solid transparent;
  border-top: 6px solid var(--color-surface);
}

.bubble-enter-active {
  transition: opacity 0.3s ease, transform 0.35s cubic-bezier(0.34, 1.56, 0.64, 1);
}
.bubble-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}
.bubble-enter-from {
  opacity: 0;
  transform: translate(-50%, -80%) scale(0.9);
}
.bubble-leave-to {
  opacity: 0;
  transform: translate(-50%, -110%) scale(0.9);
}
</style>
