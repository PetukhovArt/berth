<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { UserRound } from '../../../shared/ui/icons';
import { UiButton } from '../../../shared/ui/UiButton';
import { t } from '../../../shared/lib/i18n/i18n';
import { useScope, visibleInterval } from '../../../shared/lib/scope';
import { openChannelFor, readPresence, type ChannelPresent } from '../../../entities/session';
import { channelKeyOf } from '../../../entities/jira-issue';
import { knownMr } from '../../../entities/merge-request';
import InspectorSection from './InspectorSection.vue';

const REFRESH_MS = 30_000;

const props = defineProps<{ path: string; branch: string }>();

const scope = useScope();
const present = shallowRef<readonly ChannelPresent[] | null>(null);
const channel = computed(() => channelKeyOf(props.branch, knownMr(props.path, props.branch)?.title ?? ''));

async function refresh() {
  const key = channel.value;
  try {
    const here = await scope.load(readPresence(key));
    if (key === channel.value) present.value = here;
  } catch {
    if (key === channel.value) present.value = null;
  }
}

watch(
  channel,
  () => {
    present.value = null;
    void refresh();
  },
  { immediate: true },
);
visibleInterval(scope, REFRESH_MS, () => void refresh());
</script>

<template>
  <InspectorSection v-if="present" :title="t('inspector.people.title')" :meta="present.length ? String(present.length) : undefined" testid="inspector-people">
    <template #action>
      <UiButton type="button" class="link" :title="`# ${channel}`" data-testid="inspector-people-channel" @click="openChannelFor(channel)">{{ t('inspector.people.channel') }}</UiButton>
    </template>
    <div v-for="person in present" :key="person.member" class="row" data-testid="inspector-person">
      <UserRound class="icon" :size="13" />
      <span class="grow" :title="person.name">{{ person.name }}</span>
      <span v-if="person.agents.length" class="end agents" :title="person.agents.join(', ')">{{ t('inspector.people.agents', { agents: person.agents.join(', ') }) }}</span>
    </div>
    <p v-if="!present.length" class="note">{{ t('inspector.people.none') }}</p>
  </InspectorSection>
</template>

<style scoped>
.agents {
  max-width: 55%;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
