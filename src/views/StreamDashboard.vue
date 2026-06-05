<template>
  <div class="stream-dashboard">
    <div class="dashboard-header" data-tauri-drag-region>
      <div class="header-content">
        <h2>🎬 Stream Downloads</h2>
        <p class="subtitle">Direct capture streams managed via bundled yt-dlp &amp; ffmpeg sidecars</p>
      </div>
    </div>

    <div class="dashboard-body">
      <div v-if="tasks.length === 0" class="empty-state">
        <div class="empty-icon">⚡</div>
        <h3>No Video Streams Intercepted</h3>
        <p>Start a video stream in your browser and use the sniffer widget to send HLS/DASH streams here.</p>
      </div>

      <div v-else class="task-grid">
        <div
          v-for="task in tasks"
          :key="task.id"
          class="task-card"
          :class="{ 'card-completed': task.status === 'Completed' }"
        >
          <div class="card-top">
            <div class="title-section">
              <span class="movie-icon">🎥</span>
              <h4 class="task-title" :title="task.title">{{ task.title }}</h4>
            </div>
            <span class="quality-tag">{{ task.quality }}</span>
          </div>

          <div class="progress-section">
            <div class="progress-meta">
              <span class="percentage">{{ task.percent.toFixed(1) }}%</span>
              <span class="status-badge" :class="task.status.toLowerCase()">
                {{ task.status }}
              </span>
            </div>
            <div class="progress-bar-container">
              <div class="progress-bar-fill" :style="{ width: task.percent + '%' }"></div>
            </div>
          </div>

          <div class="card-bottom">
            <div v-if="task.status === 'Downloading'" class="metrics">
              <span class="metric-item">⚡ {{ task.speed }}</span>
              <span class="metric-divider">|</span>
              <span class="metric-item">⏳ ETA: {{ task.eta }}</span>
            </div>
            <div v-else-if="task.status === 'Completed'" class="metrics">
              <span class="metric-item success-text">✓ Completed</span>
            </div>
            <div v-else class="metrics">
              <span class="metric-item">{{ task.status }}...</span>
            </div>

            <div class="actions">
              <button
                v-if="task.status === 'Completed'"
                class="action-btn primary"
                title="Play Video"
                @click="openFile(task)"
              >
                Play
              </button>
              <button
                v-if="task.status === 'Completed'"
                class="action-btn"
                title="Show in Folder"
                @click="showInFolder(task)"
              >
                Folder
              </button>
              <button class="action-btn delete-btn" title="Remove from Dashboard" @click="removeTask(task.id)">
                ✕
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/** @fileoverview Premium Stream Downloads Monitoring View. */
import { ref, onMounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'

interface StreamTask {
  id: string
  title: string
  quality: string
  percent: number
  speed: string
  eta: string
  status: string
  outputPath: string
}

interface StreamAddedPayload {
  id: string
  title: string
  quality: string
  status: string
}

interface StreamProgressPayload {
  percent: number
  speed: string
  eta: string
  status: string
}

interface StreamStatusPayload {
  status: string
  output_path: string
}

const tasks = ref<StreamTask[]>([])

async function openFile(task: StreamTask) {
  if (!task.outputPath) return
  try {
    await invoke('open_path_normalized', { path: task.outputPath })
  } catch (err) {
    console.error('Failed to open file:', err)
  }
}

async function showInFolder(task: StreamTask) {
  if (!task.outputPath) return
  try {
    await invoke('show_item_in_dir', { path: task.outputPath })
  } catch (err) {
    console.error('Failed to show in folder:', err)
  }
}

function removeTask(id: string) {
  tasks.value = tasks.value.filter((t) => t.id !== id)
}

onMounted(async () => {
  // 1. Listen for new stream downloading tasks triggered from the Axum backend
  await listen<StreamAddedPayload>('stream-added', (event) => {
    const payload = event.payload
    const exists = tasks.value.some((t) => t.id === payload.id)
    if (exists) return

    const newTask: StreamTask = {
      id: payload.id,
      title: payload.title,
      quality: payload.quality,
      percent: 0,
      speed: '0 KB/s',
      eta: '--:--',
      status: payload.status,
      outputPath: '',
    }
    // Add new task to the top of the dashboard list
    tasks.value.unshift(newTask)

    // 2. Track download progress updates from yt-dlp child output
    listen<StreamProgressPayload>(`progress-${payload.id}`, (progressEvent) => {
      const p = progressEvent.payload
      const target = tasks.value.find((t) => t.id === payload.id)
      if (target) {
        target.percent = p.percent
        target.speed = p.speed
        target.eta = p.eta
        target.status = p.status
      }
    })

    // 3. Listen for final status completion triggering and file path maps
    listen<StreamStatusPayload>(`status-${payload.id}`, (statusEvent) => {
      const p = statusEvent.payload
      const target = tasks.value.find((t) => t.id === payload.id)
      if (target) {
        target.status = p.status
        target.outputPath = p.output_path
        if (p.status === 'Completed') {
          target.percent = 100
        }
      }
    })
  })
})
</script>

<style scoped>
.stream-dashboard {
  display: flex;
  flex-flow: column;
  height: 100%;
  background-color: var(--aside-bg);
  color: var(--m3-on-surface);
}

.dashboard-header {
  padding: 24px 28px;
  background-color: var(--m3-surface-container);
  border-bottom: 1px solid var(--m3-outline-variant);
  user-select: none;
}

.dashboard-header h2 {
  font-size: 20px;
  font-weight: 800;
  margin: 0 0 4px 0;
  letter-spacing: 0.5px;
}

.subtitle {
  font-size: 12px;
  color: var(--m3-on-surface-variant);
  margin: 0;
}

.dashboard-body {
  flex: 1;
  overflow-y: auto;
  padding: 24px 28px;
}

/* Empty State styling */
.empty-state {
  display: flex;
  flex-flow: column;
  align-items: center;
  justify-content: center;
  height: 60%;
  text-align: center;
  color: var(--m3-on-surface-variant);
  user-select: none;
}

.empty-icon {
  font-size: 48px;
  color: var(--m3-primary);
  margin-bottom: 16px;
  animation: float-pulse 3s infinite ease-in-out;
}

@keyframes float-pulse {
  0%,
  100% {
    transform: translateY(0) scale(1);
    opacity: 0.8;
  }
  50% {
    transform: translateY(-8px) scale(1.05);
    opacity: 1;
  }
}

.empty-state h3 {
  font-size: 16px;
  font-weight: 700;
  margin: 0 0 8px 0;
  color: var(--m3-on-surface);
}

.empty-state p {
  font-size: 13px;
  max-width: 320px;
  margin: 0;
  line-height: 1.5;
}

/* Task cards list layout */
.task-grid {
  display: flex;
  flex-flow: column;
  gap: 12px;
  max-width: 800px;
  margin: 0 auto;
}

.task-card {
  background: var(--m3-surface-container-low);
  border: 1px solid var(--m3-outline-variant);
  border-radius: 12px;
  padding: 16px 20px;
  transition: all 0.25s cubic-bezier(0.2, 0, 0, 1);
  display: flex;
  flex-flow: column;
  gap: 12px;
}

.task-card:hover {
  transform: translateY(-1px);
  border-color: var(--m3-primary-container);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
}

.card-completed {
  border-left: 4px solid var(--m3-primary);
}

.card-top {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
}

.title-section {
  display: flex;
  align-items: center;
  gap: 10px;
  overflow: hidden;
}

.movie-icon {
  font-size: 16px;
  flex-shrink: 0;
}

.task-title {
  font-size: 14px;
  font-weight: 700;
  margin: 0;
  color: var(--m3-on-surface);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.quality-tag {
  background: var(--m3-secondary-container);
  color: var(--m3-on-secondary-container);
  font-size: 11px;
  font-weight: 700;
  padding: 3px 8px;
  border-radius: 6px;
  flex-shrink: 0;
  user-select: none;
}

.progress-section {
  display: flex;
  flex-flow: column;
  gap: 6px;
}

.progress-meta {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 12px;
  font-weight: 600;
}

.percentage {
  color: var(--m3-on-surface);
}

.status-badge {
  text-transform: capitalize;
  font-size: 11px;
  padding: 2px 6px;
  border-radius: 4px;
  user-select: none;
}

.status-badge.starting {
  background: rgba(255, 179, 0, 0.15);
  color: #ffb300;
}

.status-badge.downloading {
  background: rgba(33, 150, 243, 0.15);
  color: #2196f3;
}

.status-badge.completed {
  background: rgba(76, 175, 80, 0.15);
  color: #4caf50;
}

.progress-bar-container {
  height: 6px;
  background: var(--m3-surface-container-highest);
  border-radius: 3px;
  overflow: hidden;
}

.progress-bar-fill {
  height: 100%;
  background: var(--m3-primary);
  border-radius: 3px;
  transition: width 0.3s cubic-bezier(0.2, 0, 0, 1);
}

.card-bottom {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
}

.metrics {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--m3-on-surface-variant);
  font-weight: 600;
}

.metric-divider {
  opacity: 0.3;
}

.success-text {
  color: #4caf50;
}

.actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.action-btn {
  background: var(--m3-surface-container-highest);
  color: var(--m3-on-surface);
  border: 1px solid var(--m3-outline-variant);
  border-radius: 6px;
  font-size: 11px;
  font-weight: 700;
  padding: 5px 12px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.action-btn:hover {
  background: var(--m3-primary-container);
  color: var(--m3-on-primary-container);
  border-color: var(--m3-primary);
}

.action-btn.primary {
  background: var(--m3-primary);
  color: var(--m3-on-primary);
  border-color: var(--m3-primary);
}

.action-btn.primary:hover {
  opacity: 0.9;
  box-shadow: 0 2px 8px rgba(var(--m3-primary-rgb), 0.3);
}

.delete-btn {
  padding: 5px 8px;
  color: var(--m3-on-surface-variant);
}

.delete-btn:hover {
  background: rgba(244, 67, 54, 0.15);
  color: #f44336;
  border-color: #f44336;
}
</style>
