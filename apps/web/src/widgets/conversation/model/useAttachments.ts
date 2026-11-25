import { computed, shallowRef } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { ATTACH_BUDGET, MAX_IMAGE_SOURCE_BYTES, MAX_TEXT_BYTES, fileUpload, uploadWeight, type AcpAttachmentUpload } from '../../../entities/session';

export type Pending = { id: number; upload: AcpAttachmentUpload };

const MB = 1024 * 1024;

export function useAttachments(images: () => boolean, notify: (text: string) => void) {
  const pending = shallowRef<readonly Pending[]>([]);
  let nextId = 0;

  const push = (upload: AcpAttachmentUpload) => (pending.value = [...pending.value, { id: nextId++, upload }]);
  const room = () => ATTACH_BUDGET - pending.value.reduce((sum, entry) => sum + uploadWeight(entry.upload), 0);

  async function add(files: readonly File[]) {
    for (const file of files) {
      const image = file.type.startsWith('image/');
      const limit = image ? MAX_IMAGE_SOURCE_BYTES : MAX_TEXT_BYTES;
      if (file.size > limit) notify(t('chat.attach.tooLarge', { name: file.name, limit: limit / MB }));
      else if (image && !images()) notify(t('chat.attach.noImages'));
      else {
        const result = await fileUpload(file, room()).catch(() => 'notText' as const);
        if (result === 'overBudget') notify(t('chat.attach.overBudget', { name: file.name }));
        else if (result === 'notText') notify(t('chat.attach.notText', { name: file.name }));
        else push(result);
      }
    }
  }

  return {
    pending,
    uploads: computed(() => pending.value.map((entry) => entry.upload)),
    add,
    addPath: (path: string) => push({ WorktreeFile: { path } }),
    remove: (id: number) => (pending.value = pending.value.filter((entry) => entry.id !== id)),
    clear: () => (pending.value = []),
  };
}
