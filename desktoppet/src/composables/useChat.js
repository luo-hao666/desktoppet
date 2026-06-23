/**
 * 聊天会话管理：流式响应监听 + 消息列表 + 知识库模式
 */

import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export function useChat() {
  const messages = ref([]) // { role: 'user'|'assistant', content, images?, files?, sources? }
  const streaming = ref(false)
  const errorMsg = ref('')
  const mode = ref('chat')  // 'chat' | 'rag'

  let unlistenChunk = null
  let unlistenDone = null
  let unlistenError = null

  function pushAssistantPlaceholder() {
    messages.value.push({ role: 'assistant', content: '' })
  }

  function appendToLastAssistant(delta) {
    const last = messages.value[messages.value.length - 1]
    if (last && last.role === 'assistant') {
      last.content += delta
    } else {
      messages.value.push({ role: 'assistant', content: delta })
    }
  }

  async function start() {
    if (unlistenChunk) return // 已订阅
    unlistenChunk = await listen('pet-chat-chunk', (e) => {
      if (
        messages.value.length === 0 ||
        messages.value[messages.value.length - 1].role !== 'assistant'
      ) {
        pushAssistantPlaceholder()
      }
      appendToLastAssistant(e.payload.delta)
    })
    unlistenDone = await listen('pet-chat-done', (e) => {
      streaming.value = false
      const sources = e.payload?.sources
      if (sources && sources.length > 0) {
        const last = messages.value[messages.value.length - 1]
        if (last && last.role === 'assistant') {
          last.sources = sources
        }
      }
    })
    unlistenError = await listen('pet-chat-error', (e) => {
      streaming.value = false
      errorMsg.value = e.payload.message || '请求失败'
      const last = messages.value[messages.value.length - 1]
      if (last && last.role === 'assistant' && last.content === '') {
        messages.value.pop()
      }
    })
  }

  function stop() {
    if (unlistenChunk) unlistenChunk()
    if (unlistenDone) unlistenDone()
    if (unlistenError) unlistenError()
    unlistenChunk = unlistenDone = unlistenError = null
  }

  /**
   * 切换聊天模式
   * @param {string} target - 'chat' | 'rag'
   */
  async function setMode(target) {
    if (target === mode.value) return
    if (streaming.value) return

    if (target === 'rag') {
      try {
        const status = await invoke('get_kb_status')
        if (!status.indexed) {
          errorMsg.value = '请先在设置中配置知识库文件夹并完成索引'
          return
        }
      } catch (e) {
        errorMsg.value = '获取知识库状态失败: ' + String(e)
        return
      }
    }

    // 切换模式 → 清空当前 Rust 侧上下文
    await invoke('clear_conversation', { ragMode: mode.value === 'rag' }).catch(() => {})

    messages.value = []
    errorMsg.value = ''
    mode.value = target
  }

  /**
   * 发送消息
   */
  async function send(text, images = [], appendedText = '') {
    if (streaming.value) return
    errorMsg.value = ''

    const userMsg = { role: 'user', content: text }
    if (images.length > 0) userMsg.images = images
    if (appendedText) userMsg.appendedText = appendedText
    messages.value.push(userMsg)

    streaming.value = true

    try {
      await invoke('send_chat', {
        message: text,
        images: images.length > 0
          ? images.map((img) => ({ base64: img.base64, mime: img.mime }))
          : null,
        appendedText: appendedText || null,
        ragMode: mode.value === 'rag',
      })
    } catch (e) {
      streaming.value = false
      errorMsg.value = String(e)
    }
  }

  async function clear() {
    try {
      await invoke('clear_conversation', { ragMode: mode.value === 'rag' })
    } catch (e) {
      console.warn('[chat] clear failed:', e)
    }
    messages.value = []
    errorMsg.value = ''
  }

  return {
    messages,
    streaming,
    errorMsg,
    mode,
    start,
    stop,
    send,
    clear,
    setMode,
  }
}
