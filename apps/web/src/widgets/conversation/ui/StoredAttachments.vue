<script setup lang="ts">
import { shallowRef, watchEffect } from 'vue';
import { AttachmentChip, attachmentName, attachmentSrc, type AcpAttachment } from '../../../entities/session';

const props = defineProps<{ attachments: readonly AcpAttachment[] }>();

const sources = shallowRef<ReadonlyMap<string, string>>(new Map());

watchEffect(() => {
  for (const attachment of props.attachments) {
    if (!('Image' in attachment)) continue;
    const blob = attachment.Image.blob;
    if (sources.value.has(blob)) continue;
    void attachmentSrc(attachment)
      .then((src) => (sources.value = new Map(sources.value).set(blob, src)))
      .catch(() => undefined);
  }
});
</script>

<template>
  <div class="attachments" data-testid="chat-attachments">
    <AttachmentChip v-for="(attachment, index) in attachments" :key="index" :label="attachmentName(attachment)" :src="'Image' in attachment ? (sources.get(attachment.Image.blob) ?? null) : null" />
  </div>
</template>

<style scoped>
.attachments {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-2);
  justify-content: flex-end;
}
</style>
