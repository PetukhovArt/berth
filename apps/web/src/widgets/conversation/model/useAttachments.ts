import { computed, shallowRef } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { MAX_UPLOAD_BYTES, fileUpload, type AcpAttachmentUpload } from '../../../entities/session';

export type Pending = { id: number; upload: AcpAttachmentUpload };

export function useAttachments(images: () => boolean, notify: (text: string) => void) {
  const pending = shallowRef<readonly Pending[]>([]);
  let nextId = 0;

  const push = (upload: AcpAttachmentUpload) => (pending.value = [...pending.value, { id: nextId++, upload }]);

  async function add(files: readonly File[]) {
    for (const file of files) {
      if (file.size > MAX_UPLOAD_BYTES) notify(t('chat.attach.tooLarge', { name: file.name }));
      else if (file.type.startsWith('image/') && !images()) notify(t('chat.attach.noImages'));
      else {
        const upload = await fileUpload(file).catch(() => null);
        if (upload) push(upload);
        else notify(t('chat.attach.notText', { name: file.name }));
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
