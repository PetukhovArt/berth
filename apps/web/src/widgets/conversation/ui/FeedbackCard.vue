<script setup lang="ts">
import { computed, useTemplateRef } from 'vue';
import { TriangleAlert } from '../../../shared/ui/icons';
import { UiButton } from '../../../shared/ui/UiButton';
import { t } from '../../../shared/lib/i18n/i18n';
import { allowRequest, alwaysAllowRequest, answerFeedbackForm, chooseOption, denyRequest, feedbackText, isYesNoPermission, offersAlways, type AcpAnswers, type FeedbackRequest, type SessionRef } from '../../../entities/session';
import type { FeedNote } from '../model/feedNotes';
import { useDigitKeys } from '../model/digitKey';
import QuestionForm from './QuestionForm.vue';

type Choice = { label: string; title?: string; primary: boolean; testid: string; run: () => Promise<void>; note: Omit<FeedNote, 'id' | 'afterKey'> };

const props = withDefaults(defineProps<{ session: SessionRef; request: FeedbackRequest; agentLabel: string; place: string; questions?: 'none' | 'choice' | 'typed'; panelId?: string }>(), { questions: 'none' });
const emit = defineEmits<{ resolved: [note: Omit<FeedNote, 'id' | 'afterKey'>] }>();

const code = computed(() => props.request.detail);
const form = computed(() => (props.questions !== 'none' && 'Agent' in props.session ? props.request.form : undefined));

async function submitForm(answers: AcpAnswers) {
  if (!form.value || !('Agent' in props.session)) return;
  const asked = form.value.questions.flatMap((question) => (answers[question.id] ?? []).map((value) => question.choices.find((choice) => choice.value === value)?.name ?? value));
  emit('resolved', { text: t('feedback.answered', { answer: asked.join(', ') }), code: null });
  await answerFeedbackForm(props.session.Agent, answers);
}

const choices = computed<Choice[]>(() => {
  const { request, session } = props;
  if (form.value) return [];
  if (!isYesNoPermission(request)) {
    return request.options.map((option, index) => ({ label: option, primary: index === 0, testid: 'feedback-option', run: () => chooseOption(session, index), note: { text: t('feedback.answered', { answer: option }), code: null } }));
  }
  const list: Choice[] = [{ label: t('feedback.allow'), primary: true, testid: 'feedback-allow', run: () => allowRequest(session), note: { text: t('feedback.allowed'), code: code.value } }];
  if (offersAlways(request)) list.push({ label: t('feedback.always'), title: t('feedback.alwaysTitle'), primary: false, testid: 'feedback-always', run: () => alwaysAllowRequest(session), note: { text: t('feedback.allowed'), code: code.value } });
  list.push({ label: t('feedback.deny'), primary: false, testid: 'feedback-deny', run: () => denyRequest(session), note: { text: t('feedback.denied'), code: code.value } });
  return list;
});

async function pick(choice: Choice) {
  emit('resolved', choice.note);
  await choice.run();
}

const card = useTemplateRef<HTMLElement>('card');

useDigitKeys(card, () => props.panelId, (index) => {
  const choice = choices.value[index];
  if (!choice) return false;
  void pick(choice);
  return true;
});
</script>

<template>
  <section ref="card" class="card" data-testid="chat-feedback" :data-kind="request.kind" :aria-label="feedbackText(request)">
    <h3 class="title"><TriangleAlert :size="15" class="icon" />{{ feedbackText(request) }}</h3>
    <code v-if="code" class="code mono" data-testid="feedback-detail">{{ code }}</code>
    <p v-if="place || request.kind === 'Question'" class="sub">{{ [place ? t('feedback.place', { place }) : '', request.kind === 'Question' ? t('feedback.asks', { agent: agentLabel }) : ''].filter(Boolean).join(' · ') }}</p>
    <QuestionForm v-if="form" :form="form" :typed="questions === 'typed'" :panel-id="panelId" @submit="submitForm" />
    <div v-else class="actions">
      <UiButton v-for="(choice, index) in choices" :key="index" type="button" class="action" :class="{ primary: choice.primary }" :title="choice.title" :data-testid="choice.testid" @click="pick(choice)">
        {{ choice.label }}
        <kbd v-if="index < 9" class="key">{{ index + 1 }}</kbd>
      </UiButton>
    </div>
  </section>
</template>

<style scoped>
.card {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: 10px;
  box-sizing: border-box;
  margin-bottom: var(--vr-space-2);
  padding: 14px var(--vr-space-4);
  background: var(--vr-color-surface-raised);
  border: 1px solid var(--vr-color-border-strong);
  border-radius: var(--vr-radius-xl);
  box-shadow: inset 3px 0 0 var(--vr-status-feedback);
}

.title {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: 0;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.icon {
  flex: none;
  color: var(--vr-status-feedback);
}

.code {
  box-sizing: border-box;
  max-height: min(30dvh, 10em);
  padding: var(--vr-space-1) var(--vr-space-2);
  overflow-x: hidden;
  overflow-y: auto;
  overflow-wrap: anywhere;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-control);
  white-space: pre-wrap;
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-chip);
}

.sub {
  margin: 0;
  font-size: var(--vr-font-size-control);
  color: var(--vr-color-text-muted);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-2);
}

.action {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-2);
  height: 30px;
  padding: 0 var(--vr-space-3);
  font: inherit;
  font-size: var(--vr-font-size-control);
  font-weight: 500;
  color: var(--vr-color-text);
  cursor: pointer;
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: var(--vr-radius-md);
}

.action.primary {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
}

.key {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-key);
  opacity: 0.6;
}
</style>
