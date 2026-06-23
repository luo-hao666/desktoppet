/**
 * 动画配置加载 + 预加载
 * 读取 config.toml，扫描文件夹，预加载所有帧为 blob URL
 */

import { readDir, readFile, readTextFile } from '@tauri-apps/plugin-fs'
import { parse } from 'smol-toml'

/**
 * 帧文件名解析：提取序号和持续毫秒
 * 格式：<前缀>_<帧序号>_<持续毫秒>.png
 */
function parseFrameFile(filename) {
  const match = filename.match(/_(\d{3})_(\d+)\.png$/i)
  if (!match) return null
  return {
    index: parseInt(match[1], 10),
    durationMs: parseInt(match[2], 10) || 125,
  }
}

/**
 * 预加载单个状态文件夹的所有帧
 */
async function loadFrames(petFolder, folder) {
  const dirPath = `${petFolder}/${folder}`
  const entries = await readDir(dirPath)

  const frames = []
  for (const entry of entries) {
    if (!entry.name?.toLowerCase().endsWith('.png')) continue
    const parsed = parseFrameFile(entry.name)
    if (!parsed) continue
    frames.push({ ...parsed, path: `${dirPath}/${entry.name}` })
  }

  // 按序号排序
  frames.sort((a, b) => a.index - b.index)

  // 读取文件内容，创建 blob URL
  const result = []
  for (const frame of frames) {
    const bytes = await readFile(frame.path)
    const blob = new Blob([bytes], { type: 'image/png' })
    const url = URL.createObjectURL(blob)
    result.push({ url, durationMs: frame.durationMs })
  }

  return result
}

/**
 * 加载角色的全部动画配置
 * @param {string} petFolder - 角色文件夹绝对路径
 * @returns {Promise<Record<string, AnimationConfig>>}
 */
export async function loadAnimationConfigs(petFolder) {
  const tomlText = await readTextFile(`${petFolder}/config.toml`)
  const raw = parse(tomlText)

  const configs = {}

  for (const [key, cfg] of Object.entries(raw)) {
    // raised 文件夹可选
    let frames = []
    try {
      frames = await loadFrames(petFolder, cfg.folder)
    } catch (e) {
      if (key === 'raised') {
        console.warn('[animation] raised 文件夹不存在，拖拽将使用 CSS 降级')
        continue
      }
      throw e
    }

    configs[key] = {
      key,
      folder: cfg.folder,
      frames,
      loop: cfg.loop,
      variants: cfg.variants || undefined,
    }

    // 预加载变体
    if (cfg.variants) {
      for (const varFolder of cfg.variants) {
        try {
          const varFrames = await loadFrames(petFolder, varFolder)
          configs[varFolder] = {
            key: varFolder,
            folder: varFolder,
            frames: varFrames,
            loop: true,
          }
        } catch (e) {
          console.warn(`[animation] 变体文件夹 ${varFolder} 加载失败:`, e)
        }
      }
    }
  }

  return configs
}
