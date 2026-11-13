<script setup lang="ts">
import { computed } from 'vue';
import { ExternalLink } from '../../../shared/ui/icons';
import { openUrl, type PipelineJob } from '../../../shared/api/connectors/api';
import { t } from '../../../shared/lib/i18n/i18n';
import { formatDuration, jobTone } from '../../../entities/merge-request';
import { StatusIcon } from '../../../entities/session';
import { useChangeRequest } from '../model/change-request-scope';
import { pipelineTone } from '../model/pipelineTone';

const { pipeline } = useChangeRequest();
const pipelineJobs = computed(() => pipeline.value?.jobs ?? []);

const stages = computed(() => {
  const groups = new Map<string, PipelineJob[]>();
  for (const job of pipelineJobs.value) {
    const list = groups.get(job.stage);
    if (list) list.push(job);
    else groups.set(job.stage, [job]);
  }
  return [...groups.entries()].map(([name, jobs]) => ({ name, jobs }));
});
</script>

<template>
  <div class="jobs" data-testid="pipeline-jobs">
    <p v-if="!pipelineJobs.length" class="empty">{{ t('forge.pipelineJobs.empty') }}</p>
    <section v-for="stage in stages" :key="stage.name" class="stage">
      <h4>{{ stage.name }}</h4>
      <a v-for="job in stage.jobs" :key="job.id" class="job" :href="job.url" :title="job.status" data-testid="pipeline-job" @click.prevent="openUrl(job.url)">
        <StatusIcon :tone="pipelineTone(jobTone(job.status))" />
        <span class="name">{{ job.name }}</span>
        <span class="status">{{ job.status }}</span>
        <span class="duration">{{ formatDuration(job.duration_s) }}</span>
        <ExternalLink :size="12" class="open vr-reveal" />
      </a>
    </section>
  </div>
</template>

<style scoped>
.jobs {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
}

.empty {
  margin: 0;
  color: var(--vr-color-text-muted);
}

.stage {
  display: flex;
  flex-direction: column;
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
}

h4 {
  height: 34px;
  margin: 0;
  padding: 0 var(--vr-space-4);
  display: flex;
  align-items: center;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  border-bottom: 1px solid var(--vr-color-border);
}

.job {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  height: 36px;
  padding: 0 var(--vr-space-4);
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
  text-decoration: none;
  cursor: pointer;
}

.job + .job {
  border-top: 1px solid var(--vr-color-border);
}

.job:hover {
  background: var(--vr-color-nav-hover);
}

.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.status {
  color: var(--vr-color-text-muted);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
}

.duration {
  width: 56px;
  text-align: right;
  color: var(--vr-color-text-subtle);
  font-size: var(--vr-font-size-xs);
}

.open {
  color: var(--vr-color-link);
  opacity: 0;
}

.job:hover .open {
  opacity: 1;
}
</style>
