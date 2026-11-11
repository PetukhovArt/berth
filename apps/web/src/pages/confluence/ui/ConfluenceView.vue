<script setup lang="ts">
import { computed } from 'vue';
import { BookOpen } from '../../../shared/ui/icons';
import { ConfluencePageView, ConfluenceSearch } from '../../../widgets/confluence';
import { t } from '../../../shared/lib/i18n/i18n';
import { withViewTransition } from '../../../shared/ui/motion/motion';
import { view } from '../../../shared/model/nav';
import { ConnectorPage } from '../../../widgets/connector-page';

const KINDS = ['confluence'] as const;

const open = computed(() => (view.value.kind === 'confluence' ? view.value.open : undefined));

function select(target: { connector: string; id: string }) {
  void withViewTransition(() => {
    view.value = { kind: 'confluence', open: target };
  });
}
</script>

<template>
  <ConnectorPage
    :kinds="KINDS"
    title="Confluence"
    :icon="BookOpen"
    :empty-text="t('confluence.view.empty')"
    :placeholder="t('confluence.view.placeholder')"
    test-id="confluence-view"
    storage-key="berth.confluence"
    picker
    :preferred="open?.connector"
    :selected="open"
  >
    <template #list="{ connector }">
      <ConfluenceSearch :key="connector" :connector="connector" @open="select" />
    </template>
    <template #detail="{ connector }">
      <ConfluencePageView v-if="open" :key="`${connector}/${open.id}`" :connector="connector" :id="open.id" @open="select" />
    </template>
  </ConnectorPage>
</template>
