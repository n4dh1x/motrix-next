<template>
  <div class="task-view">
    <header class="panel-header" data-tauri-drag-region>
      <h4 class="task-title">{{ t('nav.stream-downloads') || 'Stream Downloads' }}</h4>
    </header>

    <div class="panel-body">
      <!-- Permanent brand watermark -->
      <div class="watermark" @dragstart.prevent @selectstart.prevent>
        <img :src="watermarkSrc" alt="Motrix Next" class="watermark-brand" draggable="false" />
      </div>

      <div class="panel-content">
        <div class="stream-list">
          <div v-if="streamStore.tasks.length === 0" class="empty-state">
            <div class="empty-icon">⚡</div>
            <h3>No Video Streams Intercepted</h3>
            <p>Start a video stream in your browser and use the sniffer extension to send HLS/DASH streams here.</p>
          </div>

          <div v-else class="stream-list-inner">
            <div
              v-for="task in streamStore.tasks"
              :key="task.id"
              class="stream-task-card"
              :class="{
                'card-completed': task.status === 'Completed',
                'card-paused': task.status === 'Paused',
                'card-error': task.status === 'Error',
                'card-downloading': task.status === 'Downloading',
                'card-starting': task.status === 'Starting',
              }"
            >
              <div class="card-body">
                <div class="card-header">
                  <div class="task-title-section">
                    <span class="movie-icon">🎥</span>
                    <span class="task-name-text" :title="task.title">{{ task.title }}</span>
                    <span class="quality-tag">{{ task.quality }}</span>
                  </div>

                  <div class="task-actions">
                    <!-- Action buttons -->
                    <MTooltip v-if="task.status === 'Completed'" placement="bottom">
                      <template #trigger>
                        <button class="action-btn-circle" @click="openFile(task)">
                          <NIcon :size="16"><OpenOutline /></NIcon>
                        </button>
                      </template>
                      {{ t('task.open-file') || 'Open File' }}
                    </MTooltip>

                    <MTooltip v-if="task.status === 'Completed'" placement="bottom">
                      <template #trigger>
                        <button class="action-btn-circle" @click="showInFolder(task)">
                          <NIcon :size="16"><FolderOpenOutline /></NIcon>
                        </button>
                      </template>
                      {{ t('task.show-in-folder') || 'Show in Folder' }}
                    </MTooltip>

                    <MTooltip v-if="task.status === 'Paused' || task.status === 'Error'" placement="bottom">
                      <template #trigger>
                        <button class="action-btn-circle" @click="streamStore.resumeTask(task.id)">
                          <NIcon :size="16"><PlayOutline /></NIcon>
                        </button>
                      </template>
                      {{ t('task.resume-task') || 'Resume' }}
                    </MTooltip>

                    <MTooltip v-if="task.status === 'Downloading' || task.status === 'Starting'" placement="bottom">
                      <template #trigger>
                        <button class="action-btn-circle" @click="streamStore.pauseTask(task.id)">
                          <NIcon :size="16"><PauseOutline /></NIcon>
                        </button>
                      </template>
                      {{ t('task.pause-task') || 'Pause' }}
                    </MTooltip>

                    <MTooltip placement="bottom">
                      <template #trigger>
                        <button class="action-btn-circle dest" @click="confirmDelete(task)">
                          <NIcon :size="16"><CloseOutline /></NIcon>
                        </button>
                      </template>
                      {{ t('task.delete-task') || 'Delete Task' }}
                    </MTooltip>
                  </div>
                </div>

                <div class="task-status-slot task-status-slot--visible">
                  <div class="task-status-slot__inner">
                    <div class="task-tags task-tags--visible">
                      <span class="status-tag" :style="getStatusBadgeStyle(task.status)">
                        <NIcon :size="13">
                          <component :is="getStatusIcon(task.status)" />
                        </NIcon>
                        {{ getStatusLabel(task.status) }}
                      </span>
                    </div>
                  </div>
                </div>

                <div class="task-progress">
                  <NProgress
                    type="line"
                    :percentage="task.percent"
                    :color="getProgressColor(task.status)"
                    :height="6"
                    :border-radius="3"
                    :show-indicator="false"
                    :processing="task.status === 'Downloading' || task.status === 'Starting'"
                  />

                  <div class="task-progress-info">
                    <div class="progress-left">
                      <span>{{ task.percent.toFixed(1) }}%</span>
                    </div>
                    <div
                      class="progress-right"
                      :class="{ 'info-hidden': task.status !== 'Downloading' && task.status !== 'Starting' }"
                    >
                      <span v-if="task.eta && task.eta !== '--:--'" class="speed-text">
                        <NIcon :size="10"><TimeOutline /></NIcon>
                        <span>{{ task.eta }}</span>
                      </span>
                      <span class="speed-text">
                        <NIcon :size="10"><ArrowDownOutline /></NIcon>
                        <span>{{ task.speed }}</span>
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/** @fileoverview Premium Stream Downloads Monitoring View. */
import { computed, h, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useDialog, NProgress, NIcon, NCheckbox } from 'naive-ui'
import { useStreamStore, type StreamTask } from '@/stores/stream'
import { useTheme } from '@/composables/useTheme'
import { useAppMessage } from '@/composables/useAppMessage'
import { invoke } from '@tauri-apps/api/core'
import MTooltip from '@/components/common/MTooltip.vue'
import {
  PauseOutline,
  PlayOutline,
  FolderOpenOutline,
  OpenOutline,
  CloseOutline,
  ArrowDownOutline,
  TimeOutline,
  CheckmarkCircleOutline,
  AlertCircleOutline,
} from '@vicons/ionicons5'

import watermarkDark from '@/assets/logo-bolt-dark.png'
import watermarkLight from '@/assets/logo-bolt-light.png'

const { t } = useI18n()
const streamStore = useStreamStore()
const dialog = useDialog()
const message = useAppMessage()
const { isDark } = useTheme()

const watermarkSrc = computed(() => (isDark.value ? watermarkLight : watermarkDark))

async function openFile(task: StreamTask) {
  if (!task.outputPath) return
  try {
    const fileExists = await invoke<boolean>('check_path_exists', { path: task.outputPath })
    if (!fileExists) {
      message.warning(t('task.file-not-exist') || 'File does not exist or has been deleted')
      return
    }
    await invoke('open_path_normalized', { path: task.outputPath })
    message.success(t('task.open-file-success') || 'File opened')
  } catch (err) {
    console.error('Failed to open file:', err)
    message.warning(t('task.file-not-exist') || 'File does not exist or has been deleted')
  }
}

async function showInFolder(task: StreamTask) {
  if (!task.outputPath) return
  try {
    const fileExists = await invoke<boolean>('check_path_exists', { path: task.outputPath })
    if (!fileExists) {
      message.warning(t('task.file-not-exist') || 'File does not exist or has been deleted')
      return
    }
    await invoke('show_item_in_dir', { path: task.outputPath })
    message.success(t('task.open-folder-success') || 'Folder opened')
  } catch (err) {
    console.error('Failed to show in folder:', err)
    message.warning(t('task.file-not-exist') || 'File does not exist or has been deleted')
  }
}

function confirmDelete(task: StreamTask) {
  const deleteFiles = ref(false)
  const d = dialog.warning({
    title: t('task.delete-task') || 'Delete Task',
    content: () =>
      h('div', {}, [
        h('p', { style: 'margin: 0 0 12px; word-break: break-all;' }, task.title),
        h(
          NCheckbox,
          {
            checked: deleteFiles.value,
            'onUpdate:checked': (v: boolean) => {
              deleteFiles.value = v
            },
          },
          { default: () => t('task.delete-task-label') || 'Also delete files' },
        ),
      ]),
    positiveText: t('app.yes') || 'Yes',
    negativeText: t('app.no') || 'No',
    onPositiveClick: async () => {
      d.loading = true
      d.negativeButtonProps = { disabled: true }
      d.closable = false
      d.maskClosable = false
      try {
        await streamStore.deleteTask(task.id, deleteFiles.value)
        message.success(t('task.delete-task-success', { taskName: task.title }) || `Deleted "${task.title}"`)
      } catch (err) {
        console.error('Failed to delete task:', err)
        message.error(t('task.delete-task-fail', { taskName: task.title }) || `Failed to delete "${task.title}"`)
      }
    },
  })
}

function getStatusLabel(status: string): string {
  switch (status) {
    case 'Starting':
      return t('task.waiting') || 'Queued'
    case 'Downloading':
      return t('task.active') || 'Downloading'
    case 'Paused':
      return t('status-paused') || 'Paused'
    case 'Completed':
      return t('status-complete') || 'Complete'
    case 'Error':
      return t('status-error') || 'Error'
    default:
      return status
  }
}

function getStatusIcon(status: string) {
  switch (status) {
    case 'Completed':
      return CheckmarkCircleOutline
    case 'Error':
      return AlertCircleOutline
    case 'Paused':
      return TimeOutline
    case 'Starting':
      return TimeOutline
    default:
      return ArrowDownOutline
  }
}

function getStatusBadgeStyle(status: string) {
  switch (status) {
    case 'Completed':
      return { color: 'var(--m3-status-success, #67C23A)' }
    case 'Error':
      return { color: 'var(--m3-status-error, #F56C6C)' }
    case 'Paused':
      return { color: 'var(--m3-status-paused, #909399)' }
    case 'Starting':
      return { color: 'var(--m3-status-waiting, #ffb300)' }
    default:
      return { color: 'var(--m3-status-active, #2196f3)' }
  }
}

function getProgressColor(status: string): string {
  switch (status) {
    case 'Completed':
      return 'var(--m3-status-success, #67C23A)'
    case 'Error':
      return 'var(--m3-status-error, #F56C6C)'
    case 'Paused':
      return 'var(--m3-status-paused, #909399)'
    case 'Starting':
      return 'var(--m3-status-waiting, #ffb300)'
    default:
      return 'var(--m3-status-active, #2196f3)'
  }
}
</script>

<style scoped>
.panel-header {
  position: relative;
  padding: var(--header-top-offset) 0 12px;
  margin: 0 36px;
  border-bottom: 2px solid var(--panel-border);
  user-select: none;
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
}

.task-title {
  margin: 0;
  color: var(--panel-title);
  font-size: 16px;
  font-weight: normal;
  line-height: 24px;
  align-self: flex-start;
}

.panel-body {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.panel-content {
  padding: 0;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  position: relative;
  z-index: 1;
}

/* ── Watermark ── */
.watermark {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
  user-select: none;
  z-index: 0;
  animation: watermark-in 0.5s cubic-bezier(0.2, 0, 0, 1) both;
}

.watermark-brand {
  max-width: 480px;
  width: 80%;
  opacity: 0.35;
  user-select: none;
  -webkit-user-drag: none;
}

@keyframes watermark-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

/* ── Stream List Spacing ── */
.stream-list {
  --task-list-bottom-safety: 54px;
  padding: 16px 36px 16px;
  min-height: 100%;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
}

.stream-list-inner {
  position: relative;
  z-index: 1;
}

.stream-list-inner:not(:empty)::after {
  content: '';
  display: block;
  height: var(--task-list-bottom-safety);
}

/* ── Empty State ── */
.empty-state {
  display: flex;
  flex-flow: column;
  align-items: center;
  justify-content: center;
  height: 60%;
  text-align: center;
  color: var(--m3-on-surface-variant);
  user-select: none;
  margin-top: 60px;
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

/* ── Stream Card Styling ── */
.stream-task-card {
  position: relative;
  margin-bottom: 16px;
  background-color: var(--task-item-bg);
  border: 1px solid var(--m3-outline-variant);
  border-left: 3px solid var(--m3-outline-variant);
  border-radius: 6px;
  overflow: hidden;
  transition: border-color 0.2s cubic-bezier(0.2, 0, 0, 1);
}

.stream-task-card:hover {
  border-color: var(--task-item-hover-border);
}

.card-downloading {
  border-left-color: var(--m3-status-active, #2196f3);
}

.card-paused {
  border-left-color: var(--m3-status-paused, #909399);
}

.card-completed {
  border-left-color: var(--m3-status-success, #67c23a);
}

.card-error {
  border-left-color: var(--m3-status-error, #f56c6c);
}

.card-starting {
  border-left-color: var(--m3-status-waiting, #ffb300);
}

.card-body {
  display: flex;
  flex-direction: column;
  padding: 16px 20px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 20px;
}

.task-title-section {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  max-width: 100%;
}

.movie-icon {
  font-size: 16px;
  flex-shrink: 0;
}

.task-name-text {
  font-size: 14px;
  font-weight: 700;
  color: var(--m3-on-surface);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 480px;
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

/* ── Card Action Buttons Toolbar ── */
.task-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0 8px;
  border: 1px solid var(--m3-surface-container-highest);
  color: var(--m3-outline);
  background-color: var(--task-action-bg);
  border-radius: 18px;
  height: 32px;
  transition: all 0.2s cubic-bezier(0.2, 0, 0, 1);
}

.action-btn-circle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  color: var(--m3-outline);
  cursor: pointer;
  border-radius: 50%;
  transition:
    color 0.15s,
    background-color 0.15s,
    transform 0.2s cubic-bezier(0.2, 0, 0, 1);
}

.action-btn-circle:hover {
  color: var(--color-primary);
  background-color: var(--m3-surface-container-highest);
}

.action-btn-circle.dest:hover {
  color: var(--m3-error);
  background-color: rgba(244, 67, 54, 0.1);
}

.action-btn-circle:active {
  transform: scale(0.85);
}

/* ── Status slot ── */
.task-status-slot {
  height: 0;
  overflow: hidden;
  transition:
    height 0.42s cubic-bezier(0.05, 0.7, 0.1, 1),
    opacity 0.28s cubic-bezier(0.2, 0, 0, 1);
  opacity: 0;
}

.task-status-slot--visible {
  height: 18px;
  opacity: 1;
  margin-top: 4px;
}

.task-status-slot__inner {
  min-height: 18px;
}

.task-tags {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 18px;
  opacity: 0;
  transform: translateY(-3px);
  transition:
    opacity 0.3s cubic-bezier(0.2, 0, 0, 1),
    transform 0.3s cubic-bezier(0.05, 0.7, 0.1, 1);
  pointer-events: none;
}

.task-tags--visible {
  opacity: 1;
  transform: translateY(0);
  pointer-events: auto;
}

.status-tag {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 13px;
  opacity: 0.9;
  vertical-align: middle;
}

/* ── Progress bar section ── */
.task-progress {
  margin-top: 10px;
}

.task-progress-info {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  font-size: 12px;
  line-height: 14px;
  min-height: 14px;
  color: var(--m3-on-surface-variant);
  margin-top: 8px;
  font-variant-numeric: tabular-nums;
}

.progress-left {
  display: inline-flex;
  align-items: center;
  white-space: nowrap;
}

.progress-right {
  display: flex;
  gap: 8px;
  text-align: right;
  align-items: center;
}

.speed-text {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  font-size: 12px;
  line-height: 14px;
  white-space: nowrap;
}

.info-hidden {
  opacity: 0;
  pointer-events: none;
}
</style>
