<script setup lang="ts">
import { computed } from 'vue';
import { openUrl, type ChangeRequest, type Discussion, type PipelineJob } from '../../../shared/api/connectors/api';
import PersonAvatar from '../../../shared/ui/PersonAvatar.vue';
import RichContent from '../../../shared/ui/rich-content/RichContent.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { CommentComposer, ThreadView } from '../../../features/diff-review';
import { jobTone, pipelineLabel } from '../../../entities/merge-request';
import { StatusIcon } from '../../../entities/session';
import { useChangeRequest } from '../model/change-request-scope';
import { pipelineTone } from '../model/pipelineTone';

import { UiButton } from '../../../shared/ui/UiButton';
const props = defineProps<{ connector: string; request: ChangeRequest }>();
const emit = defineEmits<{ goto: [discussion: Discussion] }>();

const { target, discussions, pipeline, commentDraft, commenting, commentError, selectTab, upsertDiscussion, sendComment } = useChangeRequest();

const conversation = computed(() => discussions.value.filter((d) => d.notes.some((n) => !n.system)));
const anchored = computed(() => discussions.value.some((d) => d.position));
const stages = computed(() => {
  const groups = new Map<string, PipelineJob[]>();
  for (const job of pipeline.value?.jobs ?? []) {
    const list = groups.get(job.stage);
    if (list) list.push(job);
    else groups.set(job.stage, [job]);
  }
  return [...groups.entries()].map(([name, jobs]) => ({ name, jobs }));
});
</script>

<template>
  <div class="overview" data-testid="cr-overview">
    <div class="layout">
      <div class="threads-col">
        <RichContent v-if="props.request.description.trim()" :markdown="props.request.description" :connector="props.connector" variant="document" />
        <p v-else class="muted">{{ t('forge.request.noDescription') }}</p>

        <div v-if="conversation.length" class="discussions-head">
          <h3 class="section">{{ t('forge.request.discussions') }} <span class="count">{{ conversation.length }}</span></h3>
          <span v-if="anchored" class="hint vr-key-hint">{{ t('forge.request.keysHint') }}</span>
        </div>
        <div class="threads">
          <ThreadView v-for="d in conversation" :key="d.id" :discussion="d" :connector="props.connector" :target="target" show-position @goto="emit('goto', $event)" @update="upsertDiscussion" />
        </div>
        <CommentComposer
          v-model="commentDraft"
          testid="cr-comment"
          :busy="commenting"
          :error="commentError"
          :placeholder="t('forge.composer.commentPlaceholder')"
          :label="t('forge.composer.comment')"
          @send="sendComment"
          @cancel="commentError = null"
        />
      </div>

      <aside class="facts" data-testid="cr-facts">
        <section v-if="pipeline || props.request.pipeline !== 'none'" class="fact" data-testid="cr-fact-pipeline">
          <h3 class="fact-title">{{ t('forge.request.tabPipeline') }}</h3>
          <UiButton class="status-link" @click="selectTab('pipeline')">
            <StatusIcon :tone="pipelineTone(pipeline ? pipeline.status : props.request.pipeline)" />
            <span>{{ pipelineLabel(pipeline ? pipeline.status : props.request.pipeline) }}</span>
            <span v-if="pipeline" class="pipe-id">{{ t('forge.request.pipelineNumber', { id: pipeline.id }) }}</span>
          </UiButton>
          <div v-for="stage in stages" :key="stage.name" class="stage">
            <span class="stage-name">{{ stage.name }}</span>
            <a v-for="job in stage.jobs" :key="job.id" class="job" :href="job.url" :title="job.status" @click.prevent="openUrl(job.url)">
              <StatusIcon :tone="pipelineTone(jobTone(job.status))" />
              <span class="job-name">{{ job.name }}</span>
              <span class="job-status">{{ job.status }}</span>
            </a>
          </div>
        </section>

        <section v-if="props.request.reviewers.length" class="fact" data-testid="cr-fact-reviewers">
          <h3 class="fact-title">{{ t('forge.request.reviewers') }}</h3>
          <div v-for="r in props.request.reviewers" :key="r.username" class="reviewer">
            <PersonAvatar :person="r" small />
            <span class="reviewer-name">{{ r.name || r.username }}</span>
          </div>
        </section>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.overview {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 280px;
  align-items: start;
  gap: var(--vr-space-6);
  padding: var(--vr-space-5) var(--vr-space-5) var(--vr-space-6);
}

:root[data-vr-viewport='phone'] .layout {
  grid-template-columns: minmax(0, 1fr);
}

.threads-col {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-4);
  min-width: 0;
}

.muted {
  margin: 0;
  color: var(--vr-color-text-muted);
}

.discussions-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
}

.section {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.count {
  color: var(--vr-color-text-muted);
  font-weight: 400;
}

.hint {
  margin-left: auto;
  color: var(--vr-color-text-subtle);
  font-size: var(--vr-font-size-xs);
}

.threads {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-4);
}

.facts {
  position: sticky;
  top: 0;
  display: flex;
  flex-direction: column;
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-xl);
}

:root[data-vr-viewport='phone'] .facts {
  position: static;
}

.fact {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3) var(--vr-space-4);
}

.fact + .fact {
  border-top: 1px solid var(--vr-color-border);
}

.fact-title {
  margin: 0;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
}

.status-link {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  align-self: flex-start;
  padding: 0;
  color: var(--vr-color-text);
  font: inherit;
  font-size: var(--vr-font-size-sm);
  background: none;
  border: none;
  cursor: pointer;
}

.pipe-id {
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.stage {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.stage-name {
  padding-top: var(--vr-space-1);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.job {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-height: var(--vr-size-row);
  padding: 0 var(--vr-space-2);
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
  text-decoration: none;
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
}

.job:hover {
  background: var(--vr-color-nav-hover);
}

.job-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.job-status {
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.reviewer {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  font-size: var(--vr-font-size-sm);
}

.reviewer-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
</style>
