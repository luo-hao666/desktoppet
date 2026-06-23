/**
 * 面板拖拽 composable — 在 header 上 mousedown 即可拖拽面板
 * 用法: usePanelDrag(panelRef, handleRef)
 */
import { onUnmounted } from 'vue'

export function usePanelDrag(panelRef, handleRef) {
  let dragging = false
  let startX = 0
  let startY = 0
  let startLeft = 0
  let startTop = 0

  function onMouseDown(e) {
    if (e.button !== 0) return
    e.preventDefault()

    const panel = panelRef.value
    if (!panel) return

    dragging = true
    startX = e.clientX
    startY = e.clientY

    const rect = panel.getBoundingClientRect()
    startLeft = rect.left
    startTop = rect.top

    panel.style.cursor = 'grabbing'
    panel.style.userSelect = 'none'

    document.addEventListener('mousemove', onMouseMove)
    document.addEventListener('mouseup', onMouseUp)
  }

  function onMouseMove(e) {
    if (!dragging) return
    const panel = panelRef.value
    if (!panel) return

    const dx = e.clientX - startX
    const dy = e.clientY - startY

    // 边界限制，防止面板拖出窗口
    const maxX = window.innerWidth - panel.offsetWidth
    const maxY = window.innerHeight - panel.offsetHeight
    const newLeft = Math.min(Math.max(startLeft + dx, 0), maxX)
    const newTop = Math.min(Math.max(startTop + dy, 0), maxY)

    panel.style.left = newLeft + 'px'
    panel.style.top = newTop + 'px'
    panel.style.right = 'auto'
    panel.style.bottom = 'auto'
  }

  function onMouseUp() {
    dragging = false
    document.removeEventListener('mousemove', onMouseMove)
    document.removeEventListener('mouseup', onMouseUp)

    const panel = panelRef.value
    if (panel) {
      panel.style.cursor = ''
      panel.style.userSelect = ''
    }
  }

  function attach() {
    const handle = handleRef.value
    if (handle) {
      handle.addEventListener('mousedown', onMouseDown)
    }
  }

  function detach() {
    const handle = handleRef.value
    if (handle) {
      handle.removeEventListener('mousedown', onMouseDown)
    }
    document.removeEventListener('mousemove', onMouseMove)
    document.removeEventListener('mouseup', onMouseUp)
  }

  onUnmounted(detach)

  return { attach, detach }
}
