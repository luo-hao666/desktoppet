<script setup>
import { ref, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { listen } from '@tauri-apps/api/event'
import { usePanelDrag } from '../composables/usePanelDrag'

const props = defineProps({
  visible: { type: Boolean, default: false },
})
const emit = defineEmits(['close'])

const panelRef = ref(null)
const headerRef = ref(null)

const config = ref({
  provider: 'deepseek',
  api_keys: {},
  model: 'deepseek-chat',
  pet_folder: '',
  pet_size: 128,
  auto_start: false,
  pet_name: '小咪',
  kb_folder: null,
})

const saving = ref(false)
const saveMsg = ref('')

const kbStatus = ref({
  indexed: false,
  file_count: 0,
  chunk_count: 0,
  last_indexed: '',
  kb_folder: '',
  model_loaded: false,
})
const kbIndexing = ref(false)
const kbProgress = ref({ current: 0, total: 0, current_file: '' })
const kbError = ref('')

let unlistenProgress = null
let unlistenDone = null
let unlistenChatError = null
let prevKbFolder = null

let { attach: attachDrag, detach: detachDrag } = usePanelDrag(panelRef, headerRef)

const providers = [
  { id: 'deepseek', label: 'DeepSeek', models: ['deepseek-chat', 'deepseek-reasoner'] },
  { id: 'kimi', label: 'Kimi (Moonshot)', models: ['moonshot-v1-8k', 'moonshot-v1-32k', 'moonshot-v1-128k', 'kimi-k2-6'] },
  { id: 'qwen', label: '通义千问', models: ['qwen3-vl-plus', 'qwen3-vl-flash', 'qwen3.6-flash', 'qwen3.6-plus', 'qwen3.5-flash', 'qwen3.5-plus', 'qwen-vl-max', 'qwen-vl-plus'] },
]

const providersModels = {
  deepseek: ['deepseek-chat', 'deepseek-reasoner'],
  kimi: ['moonshot-v1-8k', 'moonshot-v1-32k', 'moonshot-v1-128k', 'kimi-k2-6'],
  qwen: ['qwen3-vl-plus', 'qwen3-vl-flash', 'qwen3.6-flash', 'qwen3.6-plus', 'qwen3.5-flash', 'qwen3.5-plus', 'qwen-vl-max', 'qwen-vl-plus'],
}

const currentProviderModels = ref([])
const currentApiKey = ref('')

function refreshProviderUI() {
  currentProviderModels.value = providersModels[config.value.provider] || []
  currentApiKey.value = config.value.api_keys?.[config.value.provider] || ''
}

async function loadConfig() {
  try {
    const cfg = await invoke('get_config')
    config.value = { ...config.value, ...cfg, api_keys: cfg.api_keys || {} }
    prevKbFolder = cfg.kb_folder || null
    refreshProviderUI()
    await refreshKbStatus()
  } catch (e) {
    console.warn('[settings] load config failed:', e)
  }
}

async function refreshKbStatus() {
  try {
    kbStatus.value = await invoke('get_kb_status')
  } catch (e) {
    console.warn('[settings] get_kb_status failed:', e)
  }
}

watch(() => config.value.provider, () => {
  refreshProviderUI()
  if (currentProviderModels.value.length && !currentProviderModels.value.includes(config.value.model)) {
    config.value.model = currentProviderModels.value[0]
  }
})

watch(currentApiKey, (val) => {
  if (!config.value.api_keys) config.value.api_keys = {}
  config.value.api_keys[config.value.provider] = val
})

async function save() {
  saving.value = true
  saveMsg.value = ''
  try {
    await invoke('save_config', { config: config.value })
    saveMsg.value = '已保存'
    setTimeout(() => (saveMsg.value = ''), 1500)

    if (config.value.kb_folder && config.value.kb_folder !== prevKbFolder) {
      prevKbFolder = config.value.kb_folder
      await buildIndex()
    }
  } catch (e) {
    saveMsg.value = `保存失败: ${e}`
  } finally {
    saving.value = false
  }
}

async function pickPetFolder() {
  try {
    const result = await open({
      directory: true,
      multiple: false,
      defaultPath: config.value.pet_folder || undefined,
    })
    if (result) {
      config.value.pet_folder = Array.isArray(result) ? result[0] : result
    }
  } catch (e) {
    console.warn('[settings] pickPetFolder failed:', e)
  }
}

async function pickKbFolder() {
  try {
    const result = await open({
      directory: true,
      multiple: false,
      defaultPath: config.value.kb_folder || undefined,
    })
    if (result) {
      config.value.kb_folder = Array.isArray(result) ? result[0] : result
    }
  } catch (e) {
    console.warn('[settings] pickKbFolder failed:', e)
  }
}

async function buildIndex() {
  if (!config.value.kb_folder) return
  kbError.value = ''
  kbIndexing.value = true
  try {
    await invoke('build_knowledge_base', { folder: config.value.kb_folder })
  } catch (e) {
    console.warn('[settings] build_knowledge_base failed:', e)
    kbError.value = `索引启动失败: ${e}`
    kbIndexing.value = false
  }
}

async function openIndexDir() {
  try {
    await invoke('open_index_dir')
  } catch (e) {
    console.warn('[settings] open_index_dir failed:', e)
  }
}

async function clearCurrentIndex() {
  try {
    await invoke('clear_kb_index')
    kbStatus.value = { indexed: false, file_count: 0, chunk_count: 0, last_indexed: '', kb_folder: '', model_loaded: kbStatus.value.model_loaded }
    kbError.value = ''
  } catch (e) {
    kbError.value = `清空失败: ${e}`
  }
}

async function clearAllIndexes() {
  try {
    await invoke('clear_all_indexes')
    kbStatus.value = { indexed: false, file_count: 0, chunk_count: 0, last_indexed: '', kb_folder: '', model_loaded: kbStatus.value.model_loaded }
    kbError.value = ''
  } catch (e) {
    kbError.value = `清空失败: ${e}`
  }
}

function formatIndexTime(iso) {
  if (!iso) return ''
  try {
    const d = new Date(iso)
    if (isNaN(d.getTime())) return iso
    const pad = (n) => String(n).padStart(2, '0')
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
  } catch {
    return iso
  }
}

async function setupKbListeners() {
  unlistenProgress = await listen('kb-index-progress', (e) => {
    kbProgress.value = e.payload
  })
  unlistenDone = await listen('kb-index-done', (e) => {
    kbIndexing.value = false
    kbError.value = ''
    kbStatus.value = {
      indexed: true,
      file_count: e.payload.file_count,
      chunk_count: e.payload.chunk_count,
      last_indexed: new Date().toISOString(),
      kb_folder: config.value.kb_folder || '',
    }
  })
  unlistenChatError = await listen('pet-chat-error', (e) => {
    const msg = e.payload?.message || ''
    if (msg.includes('索引') || msg.includes('嵌入') || msg.includes('模型') || msg.includes('Embedding')) {
      kbError.value = msg
      kbIndexing.value = false
    }
  })
}

function cleanupKbListeners() {
  if (unlistenProgress) unlistenProgress()
  if (unlistenDone) unlistenDone()
  if (unlistenChatError) unlistenChatError()
  unlistenProgress = null
  unlistenDone = null
  unlistenChatError = null
}

watch(() => props.visible, (val) => {
  if (val) {
    loadConfig()
    setupKbListeners()
    nextTick(() => attachDrag())
  } else {
    cleanupKbListeners()
    detachDrag()
  }
})

onMounted(() => {
  if (props.visible) {
    loadConfig()
    setupKbListeners()
    nextTick(() => attachDrag())
  }
})

onUnmounted(() => {
  cleanupKbListeners()
  detachDrag()
})
</script>

<template>
  <transition name="panel">
    <div v-if="visible" ref="panelRef" class="settings-panel">
      <div ref="headerRef" class="settings-header">
        <div class="header-left">
          <span class="header-dot"></span>
          <h3>设置</h3>
        </div>
        <button class="close-btn" @click="emit('close')">
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M1 1l12 12M13 1L1 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
        </button>
      </div>

      <div class="settings-body">
        <!-- 宠物名字 -->
        <div class="form-group">
          <label>宠物名字</label>
          <input v-model="config.pet_name" placeholder="小咪" />
        </div>

        <!-- AI Provider -->
        <div class="form-group">
          <label>AI 提供方</label>
          <select v-model="config.provider">
            <option v-for="p in providers" :key="p.id" :value="p.id">{{ p.label }}</option>
          </select>
        </div>

        <!-- API Key -->
        <div class="form-group">
          <label>API Key</label>
          <input
            v-model="currentApiKey"
            type="password"
            placeholder="填入当前 Provider 的 API Key"
          />
        </div>

        <!-- 模型 -->
        <div class="form-group">
          <label>模型</label>
          <select v-model="config.model">
            <option v-for="m in currentProviderModels" :key="m" :value="m">{{ m }}</option>
          </select>
        </div>

        <!-- 角色文件夹 -->
        <div class="form-group">
          <label>角色文件夹</label>
          <div class="row-with-btn">
            <input v-model="config.pet_folder" placeholder="留空使用默认角色" />
            <button class="small-btn" @click="pickPetFolder">浏览</button>
          </div>
        </div>

        <!-- 宠物大小 -->
        <div class="form-group">
          <label>宠物大小</label>
          <div class="size-options">
            <label v-for="s in [96, 128, 192]" :key="s" class="size-radio" :class="{ active: config.pet_size === s }">
              <input type="radio" :value="s" v-model="config.pet_size" />
              {{ s }}px
            </label>
          </div>
        </div>

        <!-- 开机自启 -->
        <div class="form-group row">
          <label>开机自启</label>
          <div class="toggle-switch" :class="{ on: config.auto_start }" @click="config.auto_start = !config.auto_start">
            <div class="toggle-knob"></div>
          </div>
        </div>

        <!-- 知识库 -->
        <div class="section-divider">知识库</div>

        <div class="form-group">
          <label>知识库文件夹</label>
          <div class="row-with-btn">
            <input v-model="config.kb_folder" placeholder="选择包含文档的文件夹" />
            <button class="small-btn" @click="pickKbFolder">浏览</button>
          </div>
        </div>

        <div class="form-group">
          <label>索引状态</label>
          <div v-if="kbError" class="kb-status error">{{ kbError }}</div>
          <div v-else-if="kbIndexing" class="kb-status">
            正在索引... {{ kbProgress.current }}/{{ kbProgress.total }} 个文件
          </div>
          <div v-else-if="kbStatus.indexed" class="kb-status success">
            已索引 {{ kbStatus.file_count }} 个文件，共 {{ kbStatus.chunk_count }} 个片段
            <span v-if="kbStatus.last_indexed"> · 上次索引 {{ formatIndexTime(kbStatus.last_indexed) }}</span>
          </div>
          <div v-else class="kb-status muted">
            <template v-if="!kbStatus.model_loaded">等待 AI 模型加载完成...</template>
            <template v-else>尚未建索引 — 请先选择文件夹并点击保存</template>
          </div>
        </div>

        <div class="form-group">
          <div class="row-with-btn">
            <button class="small-btn" :disabled="!config.kb_folder || kbIndexing || !kbStatus.model_loaded" @click="buildIndex">
              {{ kbIndexing ? '索引中...' : '重建索引' }}
            </button>
            <button class="small-btn" @click="openIndexDir">
              打开索引目录
            </button>
          </div>

          <div class="row-with-btn" style="margin-top: 6px">
            <button class="small-btn btn-danger" :disabled="!kbStatus.indexed || kbIndexing" @click="clearCurrentIndex">
              清空当前索引
            </button>
            <button class="small-btn btn-danger" :disabled="kbIndexing" @click="clearAllIndexes">
              清空所有索引
            </button>
          </div>
        </div>
      </div>

      <div class="settings-footer">
        <span class="save-msg" :class="{ show: saveMsg }">{{ saveMsg }}</span>
        <button class="primary-btn" :disabled="saving" @click="save">
          {{ saving ? '保存中...' : '保存' }}
        </button>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.settings-panel {
  position: fixed;
  top: 8px;
  right: 8px;
  width: 340px;
  max-height: calc(100vh - 16px);
  background: var(--color-surface);
  backdrop-filter: blur(24px) saturate(1.4);
  -webkit-backdrop-filter: blur(24px) saturate(1.4);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-panel);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  z-index: 50;
  pointer-events: auto;
  border: 1px solid var(--color-border);
}

.settings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  flex-shrink: 0;
  cursor: grab;
  border-bottom: 1px solid var(--color-border);
}
.settings-header:active {
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
.settings-header h3 {
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

.settings-body {
  padding: 16px;
  overflow-y: auto;
  flex: 1;
}
.settings-body::-webkit-scrollbar {
  width: 4px;
}
.settings-body::-webkit-scrollbar-thumb {
  background: rgba(0,0,0,0.1);
  border-radius: 2px;
}

.form-group {
  display: flex;
  flex-direction: column;
  margin-bottom: 14px;
}
.form-group.row {
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
}

.form-group label {
  font-size: var(--font-size-sm);
  font-weight: 500;
  color: var(--color-text-secondary);
  margin-bottom: 6px;
  text-transform: uppercase;
  letter-spacing: 0.3px;
}
.form-group.row label {
  margin-bottom: 0;
}

.form-group input[type='text'],
.form-group input[type='password'],
.form-group input:not([type]),
.form-group select {
  width: 100%;
  padding: 8px 12px;
  font-size: var(--font-size-base);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-input-bg);
  color: var(--color-text);
  outline: none;
  transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
}
.form-group input:focus,
.form-group select:focus {
  border-color: var(--color-primary-light);
  box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.1);
}
.form-group select {
  cursor: pointer;
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg width='10' height='6' viewBox='0 0 10 6' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1l4 4 4-4' stroke='%2394a3b8' stroke-width='1.5' stroke-linecap='round'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 10px center;
  padding-right: 30px;
}

.row-with-btn {
  display: flex;
  gap: 6px;
}
.row-with-btn input {
  flex: 1;
}

.small-btn {
  padding: 7px 12px;
  font-size: var(--font-size-sm);
  font-weight: 500;
  border: 1px solid var(--color-border);
  background: var(--color-surface);
  border-radius: var(--radius-md);
  cursor: pointer;
  color: var(--color-text-secondary);
  transition: all var(--transition-fast);
  white-space: nowrap;
}
.small-btn:hover:not(:disabled) {
  background: var(--color-primary-soft);
  color: var(--color-primary);
  border-color: var(--color-primary-light);
}
.small-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.small-btn.btn-danger {
  border-color: #e0c0c0;
  color: #c62828;
  background: #fff5f5;
}

.small-btn.btn-danger:hover:not(:disabled) {
  background: #ffebeb;
}

.size-options {
  display: flex;
  gap: 6px;
}
.size-radio {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 6px 14px;
  font-size: var(--font-size-sm);
  font-weight: 500;
  color: var(--color-text-secondary);
  cursor: pointer;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  transition: all var(--transition-fast);
}
.size-radio.active {
  background: var(--color-primary-soft);
  color: var(--color-primary);
  border-color: var(--color-primary-light);
}
.size-radio input {
  display: none;
}
.size-radio:hover:not(.active) {
  background: var(--color-input-bg);
}

/* toggle switch */
.toggle-switch {
  width: 40px;
  height: 24px;
  background: var(--color-border);
  border-radius: 12px;
  cursor: pointer;
  position: relative;
  transition: background var(--transition-fast);
}
.toggle-switch.on {
  background: var(--color-primary);
}
.toggle-knob {
  width: 20px;
  height: 20px;
  background: #fff;
  border-radius: 50%;
  position: absolute;
  top: 2px;
  left: 2px;
  transition: transform 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
  box-shadow: 0 1px 3px rgba(0,0,0,0.15);
}
.toggle-switch.on .toggle-knob {
  transform: translateX(16px);
}

.section-divider {
  font-size: var(--font-size-sm);
  font-weight: 600;
  color: var(--color-text);
  padding: 6px 0 12px;
  margin-bottom: 2px;
  margin-top: 4px;
  border-bottom: 1px solid var(--color-border);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.kb-status {
  font-size: var(--font-size-sm);
  padding: 8px 10px;
  background: var(--color-input-bg);
  border-radius: var(--radius-md);
  line-height: 1.5;
}
.kb-status.success { color: var(--color-success); }
.kb-status.muted { color: var(--color-text-muted); }
.kb-status.error { color: var(--color-danger); }

.settings-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
  padding: 12px 16px;
  border-top: 1px solid var(--color-border);
  flex-shrink: 0;
}
.save-msg {
  font-size: var(--font-size-sm);
  color: var(--color-success);
  opacity: 0;
  transition: opacity var(--transition-fast);
}
.save-msg.show {
  opacity: 1;
}

.primary-btn {
  padding: 8px 20px;
  background: var(--color-primary);
  color: #fff;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--font-size-base);
  font-weight: 500;
  cursor: pointer;
  transition: all var(--transition-fast);
}
.primary-btn:hover:not(:disabled) {
  background: var(--color-primary-light);
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(99, 102, 241, 0.3);
}
.primary-btn:disabled {
  background: var(--color-text-muted);
  cursor: not-allowed;
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
