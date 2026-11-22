import { computed, onScopeDispose, shallowRef, watch, type Ref } from 'vue';
import { describeError, isSearchableMessageQuery, searchConversations, type ConversationSearch } from '../../../entities/session';

const CONVERSATION_LIMIT = 500;
const DEBOUNCE_MS = 200;
const INDEX_TICK_MS = 30_000;

export function useTranscriptSearch(query: Ref<string>) {
  const found = shallowRef<ConversationSearch | null>(null);
  const answered = shallowRef('');
  const error = shallowRef<string | null>(null);
  const busy = shallowRef(false);
  const needle = computed(() => {
    const text = query.value.trim();
    return isSearchableMessageQuery(text) ? text : '';
  });
  const result = computed(() => (needle.value && answered.value === needle.value ? found.value : null));
  let run = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  function fetchNow(text: string) {
    const mine = ++run;
    busy.value = result.value === null;
    searchConversations(text, CONVERSATION_LIMIT).then(
      (answer) => {
        if (mine !== run) return;
        found.value = answer;
        answered.value = text;
        error.value = null;
        busy.value = false;
        if (!answer.remote && (answer.indexed < answer.total_files || answer.total_files === 0)) timer = setTimeout(() => fetchNow(text), INDEX_TICK_MS);
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
      found.value = null;
      answered.value = '';
      busy.value = false;
      return;
    }
    busy.value = result.value === null;
    timer = setTimeout(() => fetchNow(text), DEBOUNCE_MS);
  }, { immediate: true });

  function retry() {
    clearTimeout(timer);
    if (needle.value) fetchNow(needle.value);
  }

  onScopeDispose(() => {
    clearTimeout(timer);
    run++;
  });

  return { result, error, busy, retry };
}
