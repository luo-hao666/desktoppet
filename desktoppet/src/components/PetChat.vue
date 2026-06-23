<script setup>
import { ref, watch, nextTick, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { listen } from '@tauri-apps/api/event'
import { useChat } from '../composables/useChat'
import { usePanelDrag } from '../composables/usePanelDrag'

const props = defineProps({
  visible: { type: Boolean, default: false },
})
const emit = defineEmits(['close'])

const { messages, streaming, errorMsg, mode, start, stop, send, clear, setMode } = useChat()

const inputText = ref('')
const pendingImages = ref([])
const pendingTexts = ref([])
const messageList = ref(null)
const inputBox = ref(null)
const panelRef = ref(null)
const headerRef = ref(null)

let unlistenDragDrop = null
let { attach: attachDrag, detach: detachDrag } = usePanelDrag(panelRef, headerRef)

async function handleSend() {
  const text = inputText.value.trim()
  if (!text && pendingImages.value.length === 0 && pendingTexts.value.length === 0) return
  if (streaming.value) return

  const appendedText = pendingTexts.value
    .map((f) => `[文件 ${f.filename}]\n${f.content}`)
    .join('\n\n')

  await send(text, pendingImages.value, appendedText)

  inputText.value = ''
  pendingImages.value = []
  pendingTexts.value = []
}

function handleEnter(e) {
  if (e.shiftKey) return
  e.preventDefault()
  handleSend()
}

async function pickFile() {
  try {
    const result = await open({
      multiple: true,
      filters: [
        { name: '所有支持的文件', extensions: ['txt', 'md', 'pdf', 'docx', 'pptx', 'png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'json', 'js', 'ts', 'py', 'rs', 'html', 'css', 'csv', 'log', 'vue'] },
      ],
    })
    if (!result) return
    const paths = Array.isArray(result) ? result : [result]
    for (const path of paths) {
      await processDroppedFile(path)
    }
  } catch (e) {
    console.warn('[chat] pickFile failed:', e)
  }
}

async function processDroppedFile(path) {
  try {
    const fc = await invoke('process_file', { path })
    if (fc.file_type === 'image') {
      pendingImages.value.push({
        base64: fc.content,
        mime: fc.mime || 'image/png',
        filename: fc.filename,
      })
    } else {
      pendingTexts.value.push({
        filename: fc.filename,
        content: fc.content,
      })
    }
  } catch (e) {
    errorMsg.value = `处理文件失败: ${e}`
  }
}

function removeImage(idx) {
  pendingImages.value.splice(idx, 1)
}
function removeText(idx) {
  pendingTexts.value.splice(idx, 1)
}

async function handleClear() {
  if (streaming.value) return
  await clear()
}

async function toggleMode() {
  if (streaming.value) return
  const target = mode.value === 'chat' ? 'rag' : 'chat'
  await setMode(target)
}

async function handleClose() {
  try {
    await invoke('end_talking')
  } catch (e) {
    // ignore
  }
  emit('close')
}

async function scrollToBottom() {
  await nextTick()
  if (messageList.value) {
    messageList.value.scrollTop = messageList.value.scrollHeight
  }
}

watch(() => messages.value.length, scrollToBottom)
watch(() => messages.value[messages.value.length - 1]?.content, scrollToBottom)

watch(() => props.visible, async (val) => {
  if (val) {
    await invoke('start_talking').catch(() => {})
    await start()
    await nextTick()
    inputBox.value?.focus()
    scrollToBottom()
    // 重置面板位置
    if (panelRef.value) {
      panelRef.value.style.left = ''
      panelRef.value.style.top = ''
      panelRef.value.style.right = '8px'
      panelRef.value.style.bottom = '8px'
    }
    nextTick(() => attachDrag())
  } else {
    stop()
    detachDrag()
    if (mode.value === 'rag') {
      await setMode('chat')
    }
  }
})

onMounted(async () => {
  unlistenDragDrop = await listen('tauri://drag-drop', async (event) => {
    if (!props.visible) return
    const paths = event.payload?.paths || []
    for (const path of paths) {
      await processDroppedFile(path)
    }
  })
})

onUnmounted(() => {
  if (unlistenDragDrop) unlistenDragDrop()
  detachDrag()
  stop()
})
</script>

<template>
  <transition name="chat">
    <div v-if="visible" ref="panelRef" class="chat-panel">
      <div ref="headerRef" class="chat-header">
        <div class="header-left">
          <span class="header-dot"></span>
          <h3>与桌宠聊天</h3>
        </div>
        <div class="header-actions">
          <button class="text-btn" :disabled="streaming" @click="handleClear" title="清空对话">
            清空
          </button>
          <button class="close-btn" @click="handleClose">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M1 1l12 12M13 1L1 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
          </button>
        </div>
      </div>

      <div class="chat-body" ref="messageList">
        <div v-if="messages.length === 0" class="empty-tip">
          <div class="empty-icon">
            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"><path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z"/></svg>
          </div>
          <p>随便聊点什么吧</p>
          <span class="empty-sub">支持拖入文件或图片</span>
        </div>
        <div
          v-for="(msg, idx) in messages"
          :key="idx"
          class="msg-row"
          :class="msg.role"
          :style="{ animationDelay: `${idx * 30}ms` }"
        >
          <div class="msg-bubble">
            <div v-if="msg.images?.length" class="msg-images">
              <img
                v-for="(img, i) in msg.images"
                :key="i"
                :src="`data:${img.mime};base64,${img.base64}`"
                class="msg-image"
                :alt="img.filename"
              />
            </div>
            <div v-if="msg.appendedText" class="msg-files">
              <span class="file-tag">已附加文件内容</span>
            </div>
            <div class="msg-text">{{ msg.content }}</div>
            <div v-if="msg.sources?.length" class="msg-sources">
              参考：{{ msg.sources.join(', ') }}
            </div>
          </div>
        </div>
        <div v-if="streaming && messages[messages.length - 1]?.role !== 'assistant'" class="thinking">
          <span class="dot"></span><span class="dot"></span><span class="dot"></span>
        </div>
        <div v-if="errorMsg" class="err-msg">{{ errorMsg }}</div>
      </div>

      <div v-if="pendingImages.length || pendingTexts.length" class="pending-area">
        <div v-for="(img, i) in pendingImages" :key="`img-${i}`" class="pending-item">
          <img :src="`data:${img.mime};base64,${img.base64}`" class="pending-img" />
          <span class="pending-name">{{ img.filename }}</span>
          <button class="pending-remove" @click="removeImage(i)">×</button>
        </div>
        <div v-for="(t, i) in pendingTexts" :key="`txt-${i}`" class="pending-item">
          <span class="pending-icon">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.2"><path d="M3 1h6l4 4v8a1 1 0 01-1 1H3a1 1 0 01-1-1V2a1 1 0 011-1z"/><path d="M9 1v4h4"/></svg>
          </span>
          <span class="pending-name">{{ t.filename }}</span>
          <button class="pending-remove" @click="removeText(i)">×</button>
        </div>
      </div>

      <div class="chat-footer">
        <div class="chat-input">
          <button class="attach-btn" @click="pickFile" title="附加文件">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M10 4.5v6a3 3 0 11-6 0v-7a2 2 0 114 0v7a1 1 0 11-2 0V4.5"/></svg>
          </button>
          <textarea
            ref="inputBox"
            v-model="inputText"
            class="input-textarea"
            placeholder="输入消息..."
            rows="1"
            @keydown.enter="handleEnter"
          ></textarea>
          <button
            class="send-btn"
            :disabled="streaming"
            @click="handleSend"
          >
            <svg v-if="!streaming" width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M2 2l12 6-12 6 3-6-3-6z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
            <span v-else class="spinner"></span>
          </button>
        </div>

        <div class="mode-switch">
          <button
            class="mode-pill"
            :class="{ active: mode === 'chat' }"
            @click="mode !== 'chat' ? toggleMode() : null"
          >聊天</button>
          <button
            class="mode-pill"
            :class="{ active: mode === 'rag' }"
            @click="mode !== 'rag' ? toggleMode() : null"
          >知识库</button>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.chat-panel {
  position: fixed;
  bottom: 8px;
  right: 8px;
  width: 340px;
  height: 480px;
  background: var(--color-surface);
  backdrop-filter: blur(24px) saturate(1.4);
  -webkit-backdrop-filter: blur(24px) saturate(1.4);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-panel);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  pointer-events: auto;
  z-index: 40;
  border: 1px solid var(--color-border);
}

/* ---- header ---- */
.chat-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  flex-shrink: 0;
  cursor: grab;
  border-bottom: 1px solid var(--color-border);
}
.chat-header:active {
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
.chat-header h3 {
  margin: 0;
  font-size: var(--font-size-base);
  font-weight: 600;
  color: var(--color-text);
}
.header-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}
.text-btn {
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  padding: 4px 8px;
  cursor: pointer;
  transition: all var(--transition-fast);
}
.text-btn:hover:not(:disabled) {
  background: var(--color-primary-soft);
  color: var(--color-primary);
}
.text-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
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

/* ---- body ---- */
.chat-body {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  background: rgba(248, 250, 252, 0.5);
}
.chat-body::-webkit-scrollbar {
  width: 4px;
}
.chat-body::-webkit-scrollbar-thumb {
  background: rgba(0,0,0,0.1);
  border-radius: 2px;
}

.empty-tip {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  margin-top: 60px;
  gap: 8px;
}
.empty-icon {
  color: var(--color-text-muted);
  opacity: 0.5;
  margin-bottom: 4px;
}
.empty-tip p {
  color: var(--color-text-muted);
  font-size: var(--font-size-base);
}
.empty-sub {
  color: var(--color-text-muted);
  font-size: var(--font-size-xs);
  opacity: 0.7;
}

/* ---- messages ---- */
.msg-row {
  display: flex;
  animation: msgIn 0.3s ease-out both;
}
.msg-row.user {
  justify-content: flex-end;
}
@keyframes msgIn {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.msg-bubble {
  max-width: 80%;
  padding: 9px 13px;
  border-radius: 14px;
  font-size: var(--font-size-base);
  line-height: 1.55;
  word-wrap: break-word;
  white-space: pre-wrap;
}

.msg-row.user .msg-bubble {
  background: linear-gradient(135deg, #6366f1, #8b5cf6);
  color: #fff;
  border-bottom-right-radius: 6px;
  box-shadow: 0 2px 8px rgba(99, 102, 241, 0.25);
}
.msg-row.assistant .msg-bubble {
  background: rgba(255,255,255,0.75);
  color: var(--color-text);
  border: 1px solid var(--color-border);
  border-bottom-left-radius: 6px;
}

.msg-text {
  /* inherits from parent */
}

.msg-images {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-bottom: 6px;
}
.msg-image {
  max-width: 100px;
  max-height: 100px;
  border-radius: 8px;
  object-fit: cover;
}

.msg-files {
  margin-bottom: 4px;
}
.file-tag {
  display: inline-block;
  font-size: var(--font-size-xs);
  padding: 2px 8px;
  background: rgba(255,255,255,0.25);
  color: #fff;
  border-radius: 10px;
}
.msg-row.assistant .file-tag {
  background: var(--color-primary-soft);
  color: var(--color-primary);
}

.msg-sources {
  margin-top: 8px;
  font-size: var(--font-size-xs);
  color: var(--color-text-muted);
  border-top: 1px solid var(--color-border);
  padding-top: 6px;
}

/* ---- thinking ---- */
.thinking {
  display: flex;
  gap: 4px;
  padding: 10px 14px;
  align-self: flex-start;
  background: rgba(255,255,255,0.75);
  border-radius: 14px;
  border-bottom-left-radius: 6px;
  border: 1px solid var(--color-border);
}
.thinking .dot {
  width: 6px;
  height: 6px;
  background: var(--color-primary-light);
  border-radius: 50%;
  animation: wave 1.4s infinite ease-in-out;
}
.thinking .dot:nth-child(2) { animation-delay: 0.16s; }
.thinking .dot:nth-child(3) { animation-delay: 0.32s; }
@keyframes wave {
  0%, 60%, 100% {
    transform: translateY(0);
    opacity: 0.3;
  }
  30% {
    transform: translateY(-7px);
    opacity: 1;
  }
}

.err-msg {
  background: var(--color-danger-soft);
  color: var(--color-danger);
  padding: 8px 12px;
  border-radius: var(--radius-md);
  font-size: var(--font-size-sm);
}

/* ---- pending area ---- */
.pending-area {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 8px 12px;
  border-top: 1px solid var(--color-border);
  background: rgba(248,250,252,0.6);
  flex-shrink: 0;
}
.pending-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 4px 8px 4px 4px;
  background: rgba(255,255,255,0.9);
  border: 1px solid var(--color-border);
  border-radius: 20px;
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
  max-width: 150px;
}
.pending-img {
  width: 24px;
  height: 24px;
  object-fit: cover;
  border-radius: 50%;
}
.pending-icon {
  color: var(--color-primary);
  margin-left: 4px;
}
.pending-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 70px;
}
.pending-remove {
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  font-size: 14px;
  cursor: pointer;
  padding: 0 2px;
  line-height: 1;
  border-radius: 50%;
  transition: all var(--transition-fast);
}
.pending-remove:hover {
  color: var(--color-danger);
  background: var(--color-danger-soft);
}

/* ---- footer ---- */
.chat-footer {
  flex-shrink: 0;
  border-top: 1px solid var(--color-border);
  background: rgba(255,255,255,0.5);
}

.chat-input {
  display: flex;
  align-items: flex-end;
  gap: 6px;
  padding: 8px 10px;
}
.attach-btn {
  width: 32px;
  height: 32px;
  border: 1px solid var(--color-border);
  background: var(--color-surface);
  border-radius: 8px;
  cursor: pointer;
  color: var(--color-text-secondary);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: all var(--transition-fast);
}
.attach-btn:hover {
  background: var(--color-primary-soft);
  color: var(--color-primary);
  border-color: var(--color-primary-light);
}
.input-textarea {
  flex: 1;
  resize: none;
  padding: 7px 12px;
  font-size: var(--font-size-base);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  outline: none;
  font-family: inherit;
  line-height: 1.4;
  background: var(--color-input-bg);
  color: var(--color-text);
  transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
  max-height: 100px;
}
.input-textarea:focus {
  border-color: var(--color-primary-light);
  box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.1);
}
.input-textarea::placeholder {
  color: var(--color-text-muted);
}
.send-btn {
  width: 32px;
  height: 32px;
  background: var(--color-primary);
  color: #fff;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: all var(--transition-fast);
}
.send-btn:hover:not(:disabled) {
  background: var(--color-primary-light);
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(99, 102, 241, 0.3);
}
.send-btn:disabled {
  background: var(--color-text-muted);
  cursor: not-allowed;
}
.spinner {
  width: 14px;
  height: 14px;
  border: 2px solid rgba(255,255,255,0.3);
  border-top-color: #fff;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}
@keyframes spin {
  to { transform: rotate(360deg); }
}

/* ---- mode switch ---- */
.mode-switch {
  display: flex;
  padding: 4px 10px 8px;
  gap: 0;
  justify-content: center;
}
.mode-pill {
  padding: 4px 16px;
  font-size: var(--font-size-sm);
  font-weight: 500;
  border: 1px solid var(--color-border);
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: all var(--transition-fast);
}
.mode-pill:first-child {
  border-radius: 16px 0 0 16px;
  border-right: none;
}
.mode-pill:last-child {
  border-radius: 0 16px 16px 0;
}
.mode-pill.active {
  background: var(--color-primary);
  color: #fff;
  border-color: var(--color-primary);
}
.mode-pill:not(.active):hover {
  background: var(--color-primary-soft);
  color: var(--color-primary);
}

/* ---- transitions ---- */
.chat-enter-active {
  transition: opacity 0.25s ease, transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
}
.chat-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}
.chat-enter-from {
  opacity: 0;
  transform: scale(0.92) translateY(12px);
}
.chat-leave-to {
  opacity: 0;
  transform: scale(0.92) translateY(8px);
}
</style>
