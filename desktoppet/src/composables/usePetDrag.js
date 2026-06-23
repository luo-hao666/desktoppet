/**
 * 宠物交互处理：单击 / 双击 / 右键 / 拖拽
 *
 * 拖拽：mousedown → 长按180ms or 移动超阈值 → 进入 dragging
 * dragging 期间通过 invoke('move_window_to') 让 Rust 直接移动窗口（绕过前端 API 权限问题）
 */

import { invoke } from '@tauri-apps/api/core'

const DRAG_THRESHOLD_PX = 4
const CLICK_DOUBLE_INTERVAL_MS = 250
const HOLD_TO_DRAG_MS = 180

export function usePetInteraction(el, {
  onClick,
  onDoubleClick,
  onContextMenu,
  onDragStart,
  onDragEnd,
} = {}) {
  let pendingClickTimer = null
  let clickCount = 0

  async function handleMouseDown(e) {
    if (e.button !== 0) return

    e.preventDefault()
    const startScreenX = e.screenX
    const startScreenY = e.screenY

    // 获取窗口当前位置（通过 Rust 命令获取，确保权限）
    let initialX = 0
    let initialY = 0
    try {
      const pos = await invoke('get_window_position')
      initialX = pos.x
      initialY = pos.y
    } catch (err) {
      console.warn('[drag] get_window_position failed:', err)
      return
    }

    let isDragging = false
    let dragStartFired = false

    function fireDragStart() {
      if (dragStartFired) return
      dragStartFired = true
      onDragStart?.()
    }

    // 长按进入拖拽
    const holdTimer = setTimeout(() => {
      if (!isDragging) {
        isDragging = true
        fireDragStart()
      }
    }, HOLD_TO_DRAG_MS)

    function onMove(ev) {
      const dx = ev.screenX - startScreenX
      const dy = ev.screenY - startScreenY

      if (!isDragging) {
        if (Math.abs(dx) > DRAG_THRESHOLD_PX || Math.abs(dy) > DRAG_THRESHOLD_PX) {
          isDragging = true
          clearTimeout(holdTimer)
          fireDragStart()
        }
      }

      if (isDragging) {
        const newX = initialX + dx
        const newY = initialY + dy
        invoke('move_window_to', { x: newX, y: newY }).catch(() => {})
      }
    }

    function onUp() {
      clearTimeout(holdTimer)
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup', onUp)

      if (isDragging) {
        onDragEnd?.()
        // 保存最终位置
        invoke('get_window_position').then((pos) => {
          invoke('save_pet_position', { x: pos.x, y: pos.y }).catch(() => {})
        }).catch(() => {})
      } else {
        // 点击 or 双击
        clickCount++
        if (clickCount === 1) {
          pendingClickTimer = setTimeout(() => {
            clickCount = 0
            onClick?.()
          }, CLICK_DOUBLE_INTERVAL_MS)
        } else if (clickCount === 2) {
          if (pendingClickTimer) clearTimeout(pendingClickTimer)
          clickCount = 0
          onDoubleClick?.()
        }
      }
    }

    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)
  }

  function handleContextMenu(e) {
    e.preventDefault()
    onContextMenu?.(e)
  }

  el.addEventListener('mousedown', handleMouseDown)
  el.addEventListener('contextmenu', handleContextMenu)

  return () => {
    el.removeEventListener('mousedown', handleMouseDown)
    el.removeEventListener('contextmenu', handleContextMenu)
    if (pendingClickTimer) clearTimeout(pendingClickTimer)
  }
}
