/**
 * 逐帧动画引擎
 * 基于递归 setTimeout，支持不等长帧时长
 */

/**
 * 创建动画实例
 * @param {HTMLImageElement} imgEl - 用于渲染帧的 img 元素
 * @returns {AnimationInstance}
 */
export function createAnimation(imgEl) {
  let timer = null
  let frameIndex = 0
  let playing = false

  function scheduleFrame(config, onDone) {
    if (!playing) return
    const frame = config.frames[frameIndex]
    if (!frame) return

    // 显示当前帧
    imgEl.src = frame.url

    // 调度下一帧
    timer = setTimeout(() => {
      frameIndex++
      if (frameIndex >= config.frames.length) {
        if (config.loop) {
          frameIndex = 0
          scheduleFrame(config, onDone)
        } else {
          // 非循环动画播完
          playing = false
          onDone?.()
        }
      } else {
        scheduleFrame(config, onDone)
      }
    }, frame.durationMs)
  }

  return {
    /**
     * 播放动画
     * @param {AnimationConfig} config
     * @param {Function} [onDone] - 非循环动画播完后的回调
     */
    play(config, onDone) {
      this.stop()
      if (!config || !config.frames || config.frames.length === 0) return
      frameIndex = 0
      playing = true
      scheduleFrame(config, onDone)
    },

    /** 停止当前动画 */
    stop() {
      playing = false
      if (timer) {
        clearTimeout(timer)
        timer = null
      }
    },

    /** 当前是否正在播放 */
    isPlaying() {
      return playing
    },
  }
}
