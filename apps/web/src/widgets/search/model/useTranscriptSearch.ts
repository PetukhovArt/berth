import { computed, onScopeDispose, shallowRef, watch, type Ref } from 'vue';
import { MIN_MESSAGE_QUERY, describeError, searchMessages, type MessageSearch } from '../../../entities/session';

const MESSAGE_LIMIT = 300;
const DEBOUNCE_MS = 200;
const POLL_MS = 3000;
const MAX_POLLS = 10;

export function useTranscriptSearch(query: Ref<string>, enabled: Ref<boolean>) {
  const result = shallowRef<MessageSearch | null>(null);
  const answered = shallowRef('');
  const error = shallowRef<string | null>(null);
  const busy = shallowRef(false);
  const needle = computed(() => {
    const text = query.value.trim();
    return enabled.value && text.length >= MIN_MESSAGE_QUERY ? text : '';
  });
  const stale = computed(() => answered.value !== needle.value);
  let run = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  function fetchNow(text: string, polls: number) {
    const mine = ++run;
    busy.value = true;
    searchMessages(text, MESSAGE_LIMIT).then(
      (found) => {
        if (mine !== run) return;
        result.value = found;
        answered.value = text;
        error.value = null;
        busy.value = false;
        if (found.indexed < found.total && polls < MAX_POLLS) timer = setTimeout(() => fetchNow(text, polls + 1), POLL_MS);
      },
      (failure: unknown) => {
        if (mine !== run) return;
        error.value = describeError(failure);
        busy.value = false;
      },
    );
  }

  watch(needle, (text) => {
    clearTimeout(timer);
    run++;
    error.value = null;
    if (!text) {
      result.value = null;
      answered.value = '';
      busy.value = false;
      return;
    }
    busy.value = true;
    timer = setTimeout(() => fetchNow(text, 0), DEBOUNCE_MS);
  });

  function retry() {
    clearTimeout(timer);
    if (needle.value) fetchNow(needle.value, 0);
  }

  onScopeDispose(() => {
    clearTimeout(timer);
    run++;
  });

  return { result, error, busy, stale, retry };
}
