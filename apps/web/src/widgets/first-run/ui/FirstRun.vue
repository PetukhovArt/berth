<script setup lang="ts">
import { computed, onMounted, onUnmounted, shallowRef } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { useAiAccounts, type AiKind, type AiStatus, type AiView } from '../../../shared/api/connectors/ai';
import { ProjectActions } from '../../../features/manage-workspace';
import { addProject, HarnessIcon, useBerth } from '../../../entities/session';
import { describeError, openUrl, useConnectors } from '../../../shared/api/connectors/api';
import { historyProjects, type HistoryProject } from '../../../shared/api/fs';
import { isDesktop } from '../../../bridge';
import { withTilde } from '../../../shared/lib/homePath';
import { openSettings } from '../../../shared/model/nav';
import { chordJoiner, modifierGlyphs, onlyPrimaryPressed, primaryModifier } from '../../../shared/lib/platform';
import { Gem, Smartphone, SquareTerminal } from '../../../shared/ui/icons';
import { VButton } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import UiInput from '../../../shared/ui/UiInput.vue';
import { activityLabel } from '../model/activity';
import { dismissFirstRun, dismissPhoneRow, phoneRowDismissed } from '../model/dismissed';

const STATUS = {
  signed_in: 'connectors.ai.status.signedIn',
  signed_out: 'workbench.firstRun.agentSignedOut',
  not_installed: 'workbench.firstRun.agentNotInstalled',
  rejected: 'connectors.ai.status.rejected',
  unreachable: 'connectors.ai.status.unreachable',
  error: 'connectors.ai.status.error',
  unchecked: 'connectors.ai.status.unchecked',
} as const satisfies Record<AiStatus, string>;

const INSTALL_PAGES: Partial<Record<AiKind, string>> = {
  claude: 'https://docs.claude.com/en/docs/claude-code/setup',
  codex: 'https://github.com/openai/codex',
  gemini: 'https://github.com/google-gemini/gemini-cli',
  opencode: 'https://opencode.ai/docs',
};

const ICONS: Partial<Record<AiKind, typeof Gem>> = { gemini: Gem, opencode: SquareTerminal };
const PREVIEW = 3;
const RANK: Record<AiStatus, number> = { signed_in: 0, signed_out: 1, rejected: 1, error: 1, unreachable: 1, unchecked: 1, not_installed: 2 };

const berth = useBerth();
const { accounts, refresh, check, login } = useAiAccounts();

const cli = computed(() => accounts.value.filter((a) => a.method === 'cli').sort((a, b) => RANK[a.status] - RANK[b.status]));
const readyCount = computed(() => cli.value.filter((a) => a.status === 'signed_in').length);
const hasSignedIn = computed(() => readyCount.value > 0);
const workspaceId = computed(() => berth?.tree.value[0]?.id);

const { connectors } = useConnectors();

const found = shallowRef<HistoryProject[]>([]);
const selected = shallowRef<Set<string>>(new Set());
const adding = shallowRef(false);
const addError = shallowRef<string | null>(null);

const pathKey = (path: string) => path.replace(/[\\/]+$/, '').replace(/\\/g, '/').toLowerCase();
const known = computed(() => new Set((berth?.tree.value ?? []).flatMap((w) => w.projects.flatMap((p) => p.worktrees.map((wt) => pathKey(wt.path))))));
const projects = computed(() => found.value.filter((p) => !known.value.has(pathKey(p.path))));
const picked = computed(() => projects.value.filter((p) => selected.value.has(p.path)));
const expanded = shallowRef(false);
const shown = computed(() => (expanded.value ? projects.value : projects.value.slice(0, PREVIEW)));
const forges = computed(() => connectors.value.filter((c) => (c.kind === 'github' || c.kind === 'gitlab') && c.status === 'ok'));

const hostOf = (url: string) => {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
};
const signedIn = (account: string | undefined) => (account ? t('workbench.firstRun.signedInAs', { account }) : t('workbench.firstRun.signedIn'));

function toggle(path: string, on: boolean) {
  const next = new Set(selected.value);
  if (on) next.add(path);
  else next.delete(path);
  selected.value = next;
}

async function start() {
  const id = workspaceId.value;
  if (!id || adding.value) return;
  adding.value = true;
  addError.value = null;
  try {
    for (const project of picked.value) {
      await addProject(id, { path: project.path, name: null, createMissing: false });
      toggle(project.path, false);
    }
    dismissFirstRun();
  } catch (e) {
    addError.value = describeError(e);
  } finally {
    adding.value = false;
  }
}

const tone = (status: AiStatus) => (status === 'signed_in' ? 'ok' : status === 'rejected' || status === 'error' ? 'error' : 'idle');
const state = (a: AiView) => (a.status === 'signed_in' ? [t('workbench.firstRun.ready'), a.account, a.plan].filter(Boolean).join(' · ') : t(STATUS[a.status]));

const waitingInstall = shallowRef(false);

function install(a: AiView) {
  const page = INSTALL_PAGES[a.kind];
  if (!page) return;
  waitingInstall.value = true;
  void openUrl(page);
}

function recheckAfterInstall() {
  if (!waitingInstall.value) return;
  waitingInstall.value = false;
  void check();
}

const primaryLabel = computed(() => {
  if (adding.value) return t('workbench.firstRun.adding');
  return picked.value.length ? t('workbench.firstRun.addAndStart', { count: picked.value.length }) : t('workbench.firstRun.start');
});
const startKey = `${modifierGlyphs[primaryModifier]}${chordJoiner}Enter`;

function onKey(event: KeyboardEvent) {
  if (event.key !== 'Enter' || event.isComposing || !onlyPrimaryPressed(event)) return;
  event.preventDefault();
  if (hasSignedIn.value && workspaceId.value) void start();
}

onMounted(() => {
  void refresh();
  window.addEventListener('keydown', onKey);
  window.addEventListener('focus', recheckAfterInstall);
  historyProjects()
    .then((list) => {
      found.value = list;
      selected.value = new Set(list.slice(0, 3).map((p) => p.path));
    })
    .catch(() => undefined);
});

onUnmounted(() => {
  window.removeEventListener('keydown', onKey);
  window.removeEventListener('focus', recheckAfterInstall);
});
</script>

<template>
  <section class="first-run" data-testid="first-run">
    <div class="scroll">
      <div class="content">
        <header class="head">
          <h1 class="heading" data-testid="first-run-title">{{ t('workbench.firstRun.title') }}</h1>
          <p class="lead" data-testid="first-run-lead">{{ t('workbench.firstRun.lead') }}</p>
        </header>
        <div class="block">
          <h2 class="title">
            {{ hasSignedIn ? t('workbench.firstRun.agents') : t('workbench.firstRun.agentsSetUp') }}
            <span class="hint">{{ hasSignedIn ? t('workbench.firstRun.agentsReady', { count: readyCount }) : t('workbench.firstRun.agentsNone') }}</span>
          </h2>
          <ul class="agents" data-testid="first-run-agents">
            <li v-for="a in cli" :key="a.kind" class="agent" :data-status="a.status" :data-testid="`first-run-agent-${a.kind}`">
              <span class="tile" :data-kind="a.kind" aria-hidden="true">
                <HarnessIcon v-if="a.kind === 'claude' || a.kind === 'codex'" :harness="a.kind" :size="16" />
                <component :is="ICONS[a.kind] ?? SquareTerminal" v-else :size="16" />
              </span>
              <span class="info">
                <span class="name">{{ a.label }}</span>
                <span class="state" :data-tone="tone(a.status)">{{ state(a) }}</span>
              </span>
              <VButton v-if="a.status === 'not_installed' && INSTALL_PAGES[a.kind]" :label="t('workbench.firstRun.install')" size="small" severity="secondary" :data-testid="`first-run-install-${a.kind}`" @click="install(a)" />
              <VButton v-else-if="a.status === 'signed_out' && isDesktop" :label="t('workbench.firstRun.signIn')" size="small" :data-testid="`first-run-sign-in-${a.kind}`" @click="login(a.kind)" />
            </li>
          </ul>
        </div>
        <div v-if="projects.length" class="block">
          <div class="title-row">
            <h2 class="title">{{ t('workbench.firstRun.projects') }} <span class="hint">{{ t('workbench.firstRun.projectsHint') }}</span></h2>
            <UiButton type="button" class="link" :aria-expanded="expanded" data-testid="first-run-review-all" @click="expanded = !expanded">
              {{ expanded ? t('workbench.firstRun.showFewer') : t('workbench.firstRun.reviewAll') }}
            </UiButton>
          </div>
          <ul class="agents" data-testid="first-run-projects">
            <li v-for="p in shown" :key="p.path" class="agent" :data-testid="`first-run-project-${p.name}`">
              <label class="pick">
                <UiInput type="checkbox" class="check" :checked="selected.has(p.path)" :disabled="adding" :aria-label="p.name" data-testid="first-run-project-check" @change="toggle(p.path, ($event.target as HTMLInputElement).checked)" />
                <span class="info">
                  <span class="name">{{ p.name }}</span>
                  <span class="path" :title="p.path">{{ withTilde(p.path) }}</span>
                </span>
              </label>
              <span class="state">{{ t('workbench.firstRun.conversations', { count: p.conversations }) }} · {{ activityLabel(p.last_active) }}</span>
            </li>
          </ul>
          <ProjectActions v-if="expanded && workspaceId" class="add" :workspace-id="workspaceId" />
        </div>
        <div v-else-if="workspaceId" class="block">
          <ProjectActions class="add" :workspace-id="workspaceId" primary />
        </div>
        <div v-if="forges.length" class="block">
          <div class="title-row">
            <h2 class="title">{{ t('workbench.firstRun.mergeRequests') }} <span class="hint">{{ t('workbench.firstRun.mergeRequestsHint') }}</span></h2>
            <UiButton type="button" class="link" data-testid="first-run-connectors" @click="openSettings('connectors')">{{ t('workbench.firstRun.jiraOrConfluence') }}</UiButton>
          </div>
          <ul class="agents" data-testid="first-run-forges">
            <li v-for="c in forges" :key="c.id" class="agent" :data-testid="`first-run-forge-${c.id}`">
              <span class="name host">{{ hostOf(c.base_url) }}</span>
              <span class="state" data-tone="ok">{{ signedIn(c.account?.username) }}</span>
            </li>
          </ul>
        </div>
        <div v-if="!phoneRowDismissed" class="phone" data-testid="first-run-phone">
          <UiButton type="button" class="phone-open" :title="t('workbench.firstRun.phoneOpen')" data-testid="first-run-phone-open" @click="openSettings('remote')">
            <Smartphone :size="14" :stroke-width="1.6" />
            <span>{{ t('workbench.firstRun.phone') }}</span>
          </UiButton>
          <UiButton type="button" class="link" data-testid="first-run-phone-later" @click="dismissPhoneRow">{{ t('workbench.firstRun.phoneLater') }}</UiButton>
        </div>
      </div>
    </div>
    <footer class="footer">
      <UiButton type="button" class="skip" :disabled="adding" data-testid="first-run-start-without-agent" @click="start">{{ t('workbench.firstRun.startWithoutAgent') }}</UiButton>
      <span class="spacer" />
      <p v-if="addError" class="error" data-testid="first-run-add-error">{{ addError }}</p>
      <p v-if="!hasSignedIn" class="next" data-testid="first-run-next">{{ t('workbench.firstRun.nextStep') }}</p>
      <VButton :disabled="!hasSignedIn || adding || !workspaceId" data-testid="first-run-add-selected" @click="start">
        {{ primaryLabel }}
        <kbd class="key">{{ startKey }}</kbd>
      </VButton>
    </footer>
  </section>
</template>

<style scoped>
.first-run {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
}

.scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.content {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-6);
  width: min(100%, 45rem);
  margin: 0 auto;
  padding: var(--vr-space-8) var(--vr-space-4);
}

.head {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
}

.heading {
  margin: 0;
  font-size: var(--vr-font-size-display);
  font-weight: 600;
  color: var(--vr-color-text);
}

.lead {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.next {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
}

.block {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
}

.title {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
  color: var(--vr-color-text);
}

.agents {
  margin: 0;
  padding: 0;
  list-style: none;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
  background: var(--vr-color-surface);
}

.agent {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--vr-space-3);
  padding: var(--vr-space-3) var(--vr-space-4);
}

.agent + .agent {
  border-top: 1px solid var(--vr-color-border);
}

.tile {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  color: var(--vr-color-brand-contrast);
  background: var(--vr-color-text-muted);
  border-radius: var(--vr-radius-md);
}

.tile :deep(.harness[data-harness]) {
  color: inherit;
}

.tile[data-kind='claude'] {
  background: var(--vr-color-conflict);
}

.title-row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--vr-space-3);
}

.link {
  padding: 0;
  font: inherit;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-link);
  cursor: pointer;
  background: none;
  border: 0;
}

.link:hover {
  text-decoration: underline;
}

.host {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-sm);
}

.check {
  position: relative;
  flex: none;
  width: 16px;
  height: 16px;
  margin: 0;
  cursor: pointer;
  background: transparent;
  border: 1px solid var(--vr-color-border-strong);
  border-radius: 4px;
  appearance: none;
}

.check:checked {
  background: var(--vr-color-accent);
  border-color: var(--vr-color-accent);
}

.check:checked::after {
  position: absolute;
  top: 1px;
  left: 4px;
  width: 4px;
  height: 8px;
  content: '';
  border: solid var(--vr-color-accent-contrast);
  border-width: 0 2px 2px 0;
  transform: rotate(45deg);
}

.check:focus-visible {
  outline: 2px solid var(--vr-color-focus);
  outline-offset: 2px;
}

.check:disabled {
  cursor: default;
  opacity: 0.5;
}

.phone {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--vr-space-3);
  padding: var(--vr-space-3) var(--vr-space-4);
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.phone-open {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-3);
  min-width: 0;
  padding: 0;
  font: inherit;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
  text-align: left;
  cursor: pointer;
  background: none;
  border: 0;
}

.phone-open:hover {
  color: var(--vr-color-text);
}

.tile[data-kind='codex'] {
  background: var(--vr-color-success);
}

.agent > .info {
  flex: 1;
}

.name {
  color: var(--vr-color-text);
  font-weight: 600;
}

.state {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.state[data-tone='ok'] {
  color: var(--vr-color-success);
}

.state[data-tone='error'] {
  color: var(--vr-color-danger);
}

.hint {
  font-weight: 400;
  color: var(--vr-color-text-muted);
}

.pick {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  min-width: 0;
}

.info {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.path {
  overflow: hidden;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.error {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-danger);
}

.add {
  justify-content: flex-start;
}

.footer {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-3);
  height: 64px;
  padding: 0 var(--vr-space-6);
  background: var(--vr-color-surface);
  border-top: 1px solid var(--vr-color-border);
}

.spacer {
  flex: 1;
}

.skip {
  padding: 0;
  font: inherit;
  color: var(--vr-color-text-muted);
  cursor: pointer;
  background: none;
  border: 0;
}

.skip:hover:not(:disabled) {
  color: var(--vr-color-text);
}

.key {
  margin-left: var(--vr-space-2);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  opacity: 0.6;
}
</style>
