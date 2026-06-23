<script setup>
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { usePetStore } from '../stores/petStore'
import { createAnimation } from '../composables/useAnimation'
import { loadAnimationConfigs } from '../composables/usePetState'
import { usePetInteraction } from '../composables/usePetDrag'
import PetBubble from './PetBubble.vue'
import PetMenu from './PetMenu.vue'
import PetSettings from './PetSettings.vue'
import PetDebug from './PetDebug.vue'
import PetChat from './PetChat.vue'

const petStore = usePetStore()
const petImg = ref(null)
const loading = ref(true)
const errorMsg = ref('')

// UI 状态
const menuVisible = ref(false)
const menuPos = ref({ x: 0, y: 0 })
const settingsVisible = ref(false)
const debugVisible = ref(false)
const chatVisible = ref(false)

let anim = null
let animConfigs = {}
let idleVariantTimer = null
const unlisteners = []
let cleanupInteraction = null

function startIdleVariantCycle() {
  const idleConfig = animConfigs['idle']
  if (!idleConfig?.variants?.length) return

  function scheduleNext() {
    const delay = 20000 + Math.random() * 40000
    idleVariantTimer = setTimeout(() => {
      if (petStore.currentState !== 'idle') {
        idleVariantTimer = null
        return
      }
      const roll = Math.random()
      if (roll < 0.7) {
        anim.play(idleConfig)
      } else {
        const varFolder =
          idleConfig.variants[Math.floor(Math.random() * idleConfig.variants.length)]
        const varConfig = animConfigs[varFolder]
        if (varConfig) {
          anim.play({ ...varConfig, key: 'idle', loop: true })
        }
      }
      scheduleNext()
    }, delay)
  }
  scheduleNext()
}

function stopIdleVariantCycle() {
  if (idleVariantTimer) {
    clearTimeout(idleVariantTimer)
    idleVariantTimer = null
  }
}

function onStateChanged(newState, previousState) {
  petStore.switchState(newState, previousState)

  const config = animConfigs[newState]
  if (!config) {
    console.warn(`[pet] 状态 ${newState} 无对应动画配置`)
    return
  }

  if (!config.loop) {
    anim.play(config, () => {
      invoke('animation_finished', { stateName: newState })
    })
  } else {
    anim.play(config)
  }

  if (newState === 'idle') {
    startIdleVariantCycle()
  } else {
    stopIdleVariantCycle()
  }
}

function onDragStart() {
  const raisedConfig = animConfigs['raised']
  if (raisedConfig && anim) {
    anim.play(raisedConfig)
  } else if (petImg.value) {
    petImg.value.style.transform = 'scale(0.95)'
    petImg.value.style.filter = 'drop-shadow(2px 4px 6px rgba(0,0,0,0.3))'
    petImg.value.style.transition = 'transform 0.15s, filter 0.15s'
  }
}

function onDragEnd() {
  if (!animConfigs['raised'] && petImg.value) {
    petImg.value.style.transform = ''
    petImg.value.style.filter = ''
  }
  const config = animConfigs[petStore.currentState]
  if (config) {
    if (!config.loop) {
      anim.play(config, () => {
        invoke('animation_finished', { stateName: petStore.currentState })
      })
    } else {
      anim.play(config)
    }
  }
  // 拖拽后位置变化，立即上报新边界
  reportPetBounds()
}

function onSingleClick() {
  invoke('notify_click').catch((e) => console.warn('[pet] notify_click failed:', e))
}

function onDoubleClick() {
  chatVisible.value = true
}

function onContextMenu(e) {
  menuPos.value = { x: e.clientX, y: e.clientY }
  menuVisible.value = true
}

function onMenuSelect(id) {
  if (id === 'demo') {
    debugVisible.value = true
  } else if (id === 'settings') {
    settingsVisible.value = true
  } else if (id === 'quit') {
    invoke('trigger_shutdown')
  }
}

// === 鼠标穿透联动 ===
// 任意面板/菜单可见 → 强制不穿透；否则交给 Rust 端的光标轮询决定
function isAnyPanelOpen() {
  return menuVisible.value || settingsVisible.value || debugVisible.value || chatVisible.value
}
watch(
  [menuVisible, settingsVisible, debugVisible, chatVisible],
  () => {
    invoke('set_force_block_passthrough', { block: isAnyPanelOpen() }).catch(() => {})
  },
  { immediate: false }
)

// 上报宠物在屏幕上的中心坐标 + 外接半径（屏幕坐标），供 Rust 光标轮询判断
async function reportPetBounds() {
  if (!petImg.value) return
  try {
    const rect = petImg.value.getBoundingClientRect()
    const pos = await getCurrentWindow().outerPosition()
    const dpr = window.devicePixelRatio || 1
    const winLeft = pos.x
    const winTop = pos.y
    const centerX = Math.round(winLeft + (rect.left + rect.width / 2) * dpr)
    const centerY = Math.round(winTop + (rect.top + rect.height / 2) * dpr)
    const radius = Math.round((Math.max(rect.width, rect.height) / 2) * dpr) + 4
    await invoke('update_pet_bounds', { centerX, centerY, radius })
  } catch (e) {
    // ignore
  }
}

onMounted(async () => {
  try {
    anim = createAnimation(petImg.value)

    const petFolder = await invoke('get_default_pet_folder')
    animConfigs = await loadAnimationConfigs(petFolder)

    const initialState = await invoke('get_pet_state')
    onStateChanged(initialState, initialState)

    // 监听状态变化
    unlisteners.push(
      await listen('pet-state-changed', (event) => {
        const { state, previous } = event.payload
        onStateChanged(state, previous)
      })
    )
    // 来自托盘的命令
    unlisteners.push(
      await listen('open-settings', () => {
        settingsVisible.value = true
      })
    )
    unlisteners.push(
      await listen('open-demo', () => {
        debugVisible.value = true
      })
    )
    // 配置变更（来自设置面板保存）→ 实时同步 UI
    unlisteners.push(
      await listen('pet-config-changed', (event) => {
        const cfg = event.payload || {}
        if (cfg.pet_size) {
          petStore.petSize = cfg.pet_size
        }
        // 边界变了立即上报
        nextTick(() => reportPetBounds())
      })
    )

    cleanupInteraction = usePetInteraction(petImg.value, {
      onClick: onSingleClick,
      onDoubleClick,
      onContextMenu,
      onDragStart,
      onDragEnd,
    })

    // 同步宠物大小（从配置）
    try {
      const cfg = await invoke('get_config')
      if (cfg && cfg.pet_size) {
        petStore.petSize = cfg.pet_size
      }
    } catch (e) {
      // ignore
    }

    // 触发初始气泡（APPEAR 启动气泡）
    invoke('trigger_initial_bubble').catch(() => {})

    // 上报宠物边界，让 Rust 端光标轮询知道
    await reportPetBounds()
    // 拖拽结束后会自动 onDragEnd → 状态恢复后再上报一次
    // 这里也设个定时器以应对窗口 resize（极少发生）
    const boundsInterval = setInterval(reportPetBounds, 2000)
    unlisteners.push(() => clearInterval(boundsInterval))

    loading.value = false
  } catch (err) {
    console.error('[pet] 初始化失败:', err)
    errorMsg.value = `初始化失败: ${err}`
    loading.value = false
  }
})

onUnmounted(() => {
  stopIdleVariantCycle()
  if (anim) anim.stop()
  unlisteners.forEach((u) => u && u())
  if (cleanupInteraction) cleanupInteraction()
})
</script>

<template>
  <div class="pet-container">
    <div v-if="loading" class="pet-loading">加载中...</div>
    <div v-else-if="errorMsg" class="pet-error">{{ errorMsg }}</div>
    <img
      v-show="!loading && !errorMsg"
      ref="petImg"
      class="pet-img"
      :style="{ width: petStore.petSize + 'px', height: petStore.petSize + 'px' }"
      draggable="false"
      alt="pet"
    />

    <!-- 自动气泡：传入宠物 DOM，气泡贴在宠物头顶 -->
    <PetBubble :anchor-el="petImg" />

    <!-- 右键菜单 -->
    <PetMenu
      :visible="menuVisible"
      :x="menuPos.x"
      :y="menuPos.y"
      @close="menuVisible = false"
      @select="onMenuSelect"
    />

    <!-- 设置面板 -->
    <PetSettings :visible="settingsVisible" @close="settingsVisible = false" />

    <!-- 演示模式 -->
    <PetDebug :visible="debugVisible" @close="debugVisible = false" />

    <!-- 聊天面板 -->
    <PetChat :visible="chatVisible" @close="chatVisible = false" />
  </div>
</template>

<style scoped>
.pet-container {
  position: relative;
  width: 100%;
  height: 100%;
}

/* 宠物本体放在窗口左侧居中位置（让出右侧给聊天面板等悬浮卡片） */
.pet-img {
  position: absolute;
  top: 50%;
  left: 110px;
  transform: translate(-50%, -50%);
  cursor: pointer;
  user-select: none;
  -webkit-user-drag: none;
  image-rendering: auto;
  pointer-events: auto;
}

.pet-loading,
.pet-error {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  color: rgba(255, 255, 255, 0.8);
  background: rgba(0, 0, 0, 0.5);
  padding: 8px 16px;
  border-radius: 6px;
  font-size: 12px;
  text-align: center;
}
</style>
