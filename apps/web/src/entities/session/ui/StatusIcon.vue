<script setup lang="ts">
import { computed } from 'vue';
import type { SessionStatus } from '../model/berth';

type Tone = 'running' | 'feedback' | 'unseen' | 'finished' | 'crashed' | 'idle';
type Glyph = { tone: Tone; d: string };

const props = defineProps<{ status: SessionStatus | undefined; unseen?: boolean }>();

const RING = 'M8 2.25a5.75 5.75 0 1 0 0 11.5a5.75 5.75 0 1 0 0-11.5Z';
const CHECK = 'M5.6 8.2l1.7 1.7 3.2-3.5';

const GLYPHS: Record<SessionStatus, Glyph> = {
  fresh: { tone: 'idle', d: RING },
  running: { tone: 'running', d: 'M8 2.25a5.75 5.75 0 1 1-5.75 5.75' },
  needs_feedback: { tone: 'feedback', d: `${RING} M8 5v3.4 M8 10.9v.1` },
  finished: { tone: 'finished', d: `${RING} ${CHECK}` },
  terminated: { tone: 'crashed', d: `${RING} M6 6l4 4M10 6l-4 4` },
  disconnected: { tone: 'idle', d: 'M8 2.25a5.75 5.75 0 0 1 3.6 1.3 M13.75 8a5.75 5.75 0 0 1-1.3 3.6 M8 13.75a5.75 5.75 0 0 1-3.6-1.3 M2.25 8a5.75 5.75 0 0 1 1.3-3.6' },
  shell: { tone: 'idle', d: 'M2.25 3.25h11.5v9.5H2.25z M5 6.5l1.8 1.5L5 9.5 M8.5 10h2.5' },
  closed: { tone: 'idle', d: `${RING} M5.5 8h5` },
};

const UNSEEN: Glyph = { tone: 'unseen', d: `M13.7 7.2A5.75 5.75 0 1 1 8.8 2.3 ${CHECK} M12.9 1.9a1.3 1.3 0 1 0 .01 0Z` };

const glyph = computed<Glyph>(() => {
  if (props.status === 'finished' && props.unseen) return UNSEEN;
  return props.status ? GLYPHS[props.status] : { tone: 'idle', d: RING };
});
</script>

<template>
  <svg class="status-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" :data-status="status" :data-tone="glyph.tone">
    <path :d="glyph.d" />
  </svg>
</template>

<style scoped>
.status-icon {
  flex: none;
  width: 13px;
  height: 13px;
  color: var(--vr-status-idle);
}

.status-icon[data-tone='running'] {
  color: var(--vr-status-running);
}

.status-icon[data-tone='feedback'] {
  color: var(--vr-status-feedback);
}

.status-icon[data-tone='unseen'] {
  color: var(--vr-status-unseen);
}

.status-icon[data-tone='finished'] {
  color: var(--vr-status-finished);
}

.status-icon[data-tone='crashed'] {
  color: var(--vr-status-crashed);
}
</style>
