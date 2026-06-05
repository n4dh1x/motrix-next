/** @fileoverview Pinia store for stream downloads management and persistence. */
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { load } from '@tauri-apps/plugin-store'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { useHistoryStore } from '@/stores/history'
import { logger } from '@shared/logger'

export interface StreamTask {
  id: string
  title: string
  quality: string
  percent: number
  speed: string
  eta: string
  status: 'Starting' | 'Downloading' | 'Paused' | 'Completed' | 'Error'
  outputPath: string
  url: string
  headers?: {
    'User-Agent'?: string
    Referer?: string
    Cookie?: string
  }
  addedAt: string
}

export interface StreamAddedPayload {
  id: string
  title: string
  quality: string
  status: 'Starting' | 'Downloading' | 'Paused' | 'Completed' | 'Error'
  url: string
  headers?: {
    'User-Agent'?: string
    Referer?: string
    Cookie?: string
  }
}

export interface StreamProgressPayload {
  taskId: string
  percent: number
  speed: string
  eta: string
  status: 'Starting' | 'Downloading' | 'Paused' | 'Completed' | 'Error'
}

export interface StreamStatusPayload {
  taskId: string
  status: 'Starting' | 'Downloading' | 'Paused' | 'Completed' | 'Error'
  outputPath: string
}

let storePromise: ReturnType<typeof load> | null = null
async function getStore() {
  if (!storePromise) {
    storePromise = load('stream_downloads.json')
  }
  return await storePromise
}

export const useStreamStore = defineStore('stream', () => {
  const tasks = ref<StreamTask[]>([])
  let isInitialized = false

  async function loadTasks() {
    try {
      const store = await getStore()
      const saved = await store.get<StreamTask[]>('tasks')
      if (saved && Array.isArray(saved)) {
        // Map any active/starting tasks back to paused since they terminated
        tasks.value = saved.map((t) => {
          if (t.status === 'Downloading' || t.status === 'Starting') {
            return { ...t, status: 'Paused', speed: '0 B/s', eta: '--:--' }
          }
          return t
        })
      }
    } catch (e) {
      logger.error('StreamStore.loadTasks', e)
    }
  }

  async function saveTasks() {
    try {
      const store = await getStore()
      await store.set('tasks', tasks.value)
      await store.save()
    } catch (e) {
      logger.error('StreamStore.saveTasks', e)
    }
  }

  async function pauseTask(taskId: string) {
    const task = tasks.value.find((t) => t.id === taskId)
    if (!task) return

    try {
      await invoke('stream_pause', { taskId })
      task.status = 'Paused'
      task.speed = '0 B/s'
      task.eta = '--:--'
      await saveTasks()
    } catch (e) {
      logger.error('StreamStore.pauseTask', e)
    }
  }

  async function resumeTask(taskId: string) {
    const task = tasks.value.find((t) => t.id === taskId)
    if (!task) return

    try {
      task.status = 'Starting'
      task.speed = '0 B/s'
      task.eta = '--:--'
      await saveTasks()
      await invoke('stream_resume', {
        taskId,
        url: task.url,
        title: task.title,
        quality: task.quality,
        headers: task.headers,
      })
    } catch (e) {
      logger.error('StreamStore.resumeTask', e)
      task.status = 'Error'
      await saveTasks()
    }
  }

  async function deleteTask(taskId: string, deleteFile: boolean) {
    const task = tasks.value.find((t) => t.id === taskId)
    if (!task) return

    try {
      await invoke('stream_delete', {
        taskId,
        deleteFile,
        outputPath: task.outputPath || null,
      })

      // Remove from history DB if it was completed
      if (task.status === 'Completed') {
        const historyStore = useHistoryStore()
        await historyStore.removeRecord(taskId)
      }

      tasks.value = tasks.value.filter((t) => t.id !== taskId)
      await saveTasks()
    } catch (e) {
      logger.error('StreamStore.deleteTask', e)
    }
  }

  function initListeners() {
    if (isInitialized) return
    isInitialized = true

    // 1. Listen for new tasks
    listen<StreamAddedPayload>('stream-added', async (event) => {
      const payload = event.payload
      const target = tasks.value.find((t) => t.id === payload.id)
      if (target) {
        target.status = payload.status
      } else {
        const newTask: StreamTask = {
          id: payload.id,
          title: payload.title,
          quality: payload.quality,
          percent: 0,
          speed: '0 B/s',
          eta: '--:--',
          status: payload.status,
          outputPath: '',
          url: payload.url,
          headers: payload.headers,
          addedAt: new Date().toISOString(),
        }
        tasks.value.unshift(newTask)
      }
      await saveTasks()
    })

    // 2. Listen for download progress updates
    listen<StreamProgressPayload>('stream-progress', (event) => {
      const p = event.payload
      const target = tasks.value.find((t) => t.id === p.taskId)
      if (target) {
        target.percent = p.percent
        target.speed = p.speed
        target.eta = p.eta
        target.status = p.status
      }
    })

    // 3. Listen for final status completion/error
    listen<StreamStatusPayload>('stream-status', async (event) => {
      const p = event.payload
      const target = tasks.value.find((t) => t.id === p.taskId)
      if (target) {
        target.status = p.status
        target.outputPath = p.outputPath
        if (p.status === 'Completed') {
          target.percent = 100

          // Bridge completed stream download to regular history store
          const historyStore = useHistoryStore()
          const dir = p.outputPath ? p.outputPath.substring(0, p.outputPath.lastIndexOf('/')) : undefined

          try {
            await historyStore.addRecord({
              gid: target.id,
              name: target.title,
              uri: target.url,
              dir,
              total_length: 0, // yt-dlp doesn't report exact file length to Rust easily, 0 is fine
              status: 'complete',
              task_type: 'stream',
              added_at: target.addedAt,
              completed_at: new Date().toISOString(),
            })
          } catch (err) {
            logger.error('StreamStore.bridgeToHistory', err)
          }
        }
      }
      await saveTasks()
    })
  }

  return {
    tasks,
    loadTasks,
    saveTasks,
    pauseTask,
    resumeTask,
    deleteTask,
    initListeners,
  }
})
