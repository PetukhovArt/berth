<script setup lang="ts">
import { nextTick, onMounted, ref } from 'vue';
import Badge from 'primevue/badge';
import Checkbox from 'primevue/checkbox';
import ProgressBar from 'primevue/progressbar';
import SelectButton from 'primevue/selectbutton';
import Tab from 'primevue/tab';
import TabList from 'primevue/tablist';
import TabPanel from 'primevue/tabpanel';
import TabPanels from 'primevue/tabpanels';
import Tabs from 'primevue/tabs';
import ToggleButton from 'primevue/togglebutton';
import Tooltip from 'primevue/tooltip';
import { DARK_CLASS, VButton, VChipGroup, VContextMenu, VDialog, VInputText, VMessage, VMultiSelect, VPopover, VSelect, VTag, VTextarea, VToggleSwitch } from '../src';
import DialogActions from '../../../apps/web/src/shared/ui/DialogActions.vue';
import ConfirmPopover from '../../../apps/web/src/shared/ui/ConfirmPopover.vue';
import Section from './Section.vue';
import Cell from './Cell.vue';

const vTooltip = Tooltip;

type Severity = 'secondary' | 'success' | 'info' | 'warn' | 'danger' | 'help' | 'contrast' | undefined;
const SEVERITIES: { label: string; value: Severity }[] = [
  { label: 'primary', value: undefined },
  { label: 'secondary', value: 'secondary' },
  { label: 'success', value: 'success' },
  { label: 'info', value: 'info' },
  { label: 'warn', value: 'warn' },
  { label: 'danger', value: 'danger' },
  { label: 'help', value: 'help' },
  { label: 'contrast', value: 'contrast' },
];
const TAG_SEVERITIES = ['secondary', 'success', 'info', 'warn', 'danger', 'contrast'] as const;
const MESSAGE_SEVERITIES = ['success', 'info', 'warn', 'error', 'secondary', 'contrast'] as const;

const SHORT = [
  { label: 'Claude', value: 'claude' },
  { label: 'Codex', value: 'codex' },
  { label: 'Cursor', value: 'cursor' },
];
const LONG = [
  'Amsterdam',
  'Barcelona',
  'Berlin',
  'Brussels',
  'Budapest',
  'Copenhagen',
  'Dublin',
  'Helsinki',
  'Lisbon',
  'Madrid',
  'Oslo',
  'Paris',
  'Prague',
  'Rome',
  'Vienna',
].map((label) => ({ label, value: label.toLowerCase() }));

const dark = ref(!new URLSearchParams(location.search).has('light') && !location.hash.includes('light'));
document.documentElement.classList.toggle(DARK_CLASS, dark.value);

function toggleTheme() {
  dark.value = !dark.value;
  document.documentElement.classList.toggle(DARK_CLASS, dark.value);
}

const text = ref('');
const textarea = ref('');
const shortValue = ref('codex');
const longValue = ref('berlin');
const emptyValue = ref<string | null>(null);
const multi = ref(['berlin', 'oslo']);
const toggleOn = ref(true);
const toggleOff = ref(false);
const chip = ref('all');
const chipOptions = [
  { value: 'all', label: 'All', count: 12 },
  { value: 'open', label: 'Open', count: 4 },
  { value: 'merged', label: 'Merged', count: 0 },
  { value: 'draft', label: 'Draft' },
];
const tab = ref('overview');
const segmented = ref('split');
const segmentedOptions = [
  { label: 'Code', value: 'code' },
  { label: 'Split', value: 'split' },
  { label: 'Preview', value: 'preview' },
];
const pressed = ref(true);
const checked = ref(true);
const dialogOpen = ref(false);

const menuItems = [{ label: 'Rename', command: () => undefined }, { label: 'Move to worktree', command: () => undefined }, { separator: true }, { label: 'Close', command: () => undefined }];

const popover = ref<InstanceType<typeof VPopover>>();
const popoverAnchor = ref<InstanceType<typeof VButton>>();
const confirm = ref<InstanceType<typeof ConfirmPopover>>();
const confirmAnchor = ref<InstanceType<typeof VButton>>();
const menu = ref<InstanceType<typeof VContextMenu>>();
const menuAnchor = ref<HTMLElement>();
const tooltipAnchor = ref<HTMLElement>();

function press(root: Element, selector: string, key: string) {
  root.querySelector(selector)?.dispatchEvent(new KeyboardEvent('keydown', { key, code: key, bubbles: true, cancelable: true }));
}

function filterWith(cell: Element, query: string) {
  const input = cell.querySelector<HTMLInputElement>('.p-select-filter, .p-multiselect-filter');
  if (!input) return;
  input.value = query;
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

function openOverlays() {
  for (const cell of document.querySelectorAll('[data-pin="select"]')) press(cell, '[role="combobox"]', 'ArrowDown');
  for (const cell of document.querySelectorAll('[data-pin="multiselect"]')) press(cell, 'input[role="combobox"]', 'ArrowDown');
  setTimeout(() => {
    const longCell = document.querySelector('[data-pin-cell="long"]');
    if (longCell) filterWith(longCell, 'ar');
  }, 250);
  if (popoverAnchor.value) popover.value?.show(new MouseEvent('click'), popoverAnchor.value.$el);
  if (confirmAnchor.value) confirm.value?.show(confirmAnchor.value.$el);
  if (menuAnchor.value) {
    const rect = menuAnchor.value.getBoundingClientRect();
    menu.value?.show(new MouseEvent('contextmenu', { clientX: rect.left, clientY: rect.bottom }));
  }
  tooltipAnchor.value?.dispatchEvent(new MouseEvent('mouseenter'));
}

function reopen() {
  setTimeout(openOverlays, 80);
}

onMounted(async () => {
  await nextTick();
  setTimeout(() => {
    openOverlays();
    setTimeout(() => window.scrollTo(0, 0), 700);
  }, 400);
});
</script>

<template>
  <main class="gallery">
    <header class="gallery-head">
      <h1>Berth UI kit gallery</h1>
      <span class="spacer" />
      <VButton label="Reopen overlays" size="small" severity="secondary" data-testid="gallery-reopen" @click="reopen" />
      <VButton :label="dark ? 'Switch to light' : 'Switch to dark'" size="small" severity="secondary" data-testid="gallery-theme" @click="toggleTheme" />
    </header>
    <p class="gallery-intro">
      PrimeVue, the Berth preset and tokens (packages/ui-kit/src/index.ts), the kit wrappers, then the app. This page mounts the same preset, tokens and app.css as apps/web. Hover and focus columns are forced with the
      pseudo-hover and pseudo-focus classes.
    </p>
    <nav class="gallery-nav">
      <a href="#button">VButton</a>
      <a href="#input">VInputText / VTextarea</a>
      <a href="#select">VSelect</a>
      <a href="#multiselect">VMultiSelect</a>
      <a href="#toggle">VToggleSwitch</a>
      <a href="#chips">VChipGroup</a>
      <a href="#tag">VTag</a>
      <a href="#message">VMessage</a>
      <a href="#dialog">VDialog / DialogActions</a>
      <a href="#popover">VPopover / ConfirmPopover</a>
      <a href="#menu">VContextMenu</a>
      <a href="#candidates">Not in kit</a>
    </nav>

    <Section id="button" title="VButton" base="PrimeVue Button" adds="Re-export. The preset sets radius, gap, the small size and a danger severity built from the btn-danger tokens; button.css fixes icon-only padding.">
      <p class="row-title">Severity by variant</p>
      <div class="matrix">
        <span />
        <span class="head">solid</span>
        <span class="head">outlined</span>
        <span class="head">text</span>
        <template v-for="severity in SEVERITIES" :key="severity.label">
          <span class="head">{{ severity.label }}</span>
          <VButton :label="severity.label" :severity="severity.value" />
          <VButton :label="severity.label" :severity="severity.value" outlined />
          <VButton :label="severity.label" :severity="severity.value" text />
        </template>
      </div>
      <p class="row-title">Sizes</p>
      <div class="row">
        <Cell label="small"><VButton label="Small" size="small" /></Cell>
        <Cell label="default"><VButton label="Default" /></Cell>
        <Cell label="large"><VButton label="Large" size="large" /></Cell>
        <Cell label="small secondary"><VButton label="Small" size="small" severity="secondary" /></Cell>
        <Cell label="small text"><VButton label="Small" size="small" severity="secondary" text /></Cell>
        <Cell label="icon only (menu trigger)"><VButton label="⋯" size="small" text severity="secondary" aria-label="More" /></Cell>
        <Cell label="rounded"><VButton label="Rounded" rounded /></Cell>
      </div>
      <p class="row-title">States</p>
      <div class="row">
        <template v-for="severity in [SEVERITIES[0], SEVERITIES[1], SEVERITIES[5]]" :key="severity.label">
          <Cell :label="`${severity.label} default`"><VButton label="Action" :severity="severity.value" /></Cell>
          <Cell :label="`${severity.label} hover`"><VButton label="Action" :severity="severity.value" class="pseudo-hover" /></Cell>
          <Cell :label="`${severity.label} focus`"><VButton label="Action" :severity="severity.value" class="pseudo-focus" /></Cell>
          <Cell :label="`${severity.label} active`"><VButton label="Action" :severity="severity.value" class="pseudo-active" /></Cell>
          <Cell :label="`${severity.label} disabled`"><VButton label="Action" :severity="severity.value" disabled /></Cell>
          <Cell :label="`${severity.label} loading`"><VButton label="Action" :severity="severity.value" loading /></Cell>
        </template>
      </div>
    </Section>

    <Section id="input" title="VInputText · VTextarea" base="PrimeVue InputText · Textarea" adds="Re-exports. Field colors, hover and focus borders come from the preset formField tokens.">
      <div class="row">
        <Cell class="field" label="default"><VInputText v-model="text" placeholder="Placeholder" /></Cell>
        <Cell class="field" label="hover"><VInputText placeholder="Placeholder" class="pseudo-hover" /></Cell>
        <Cell class="field" label="focus"><VInputText placeholder="Placeholder" class="pseudo-focus" /></Cell>
        <Cell class="field" label="filled"><VInputText model-value="feature/search-filter" /></Cell>
        <Cell class="field" label="disabled"><VInputText model-value="Disabled value" disabled /></Cell>
        <Cell class="field" label="invalid (error)"><VInputText model-value="bad value" invalid /></Cell>
        <Cell class="field" label="small"><VInputText placeholder="Small" size="small" /></Cell>
        <Cell class="field" label="large"><VInputText placeholder="Large" size="large" /></Cell>
      </div>
      <div class="row">
        <Cell class="field" label="textarea default"><VTextarea v-model="textarea" rows="3" placeholder="Placeholder" /></Cell>
        <Cell class="field" label="textarea focus"><VTextarea rows="3" placeholder="Placeholder" class="pseudo-focus" /></Cell>
        <Cell class="field" label="textarea disabled"><VTextarea rows="3" model-value="Disabled" disabled /></Cell>
        <Cell class="field" label="textarea invalid"><VTextarea rows="3" model-value="Wrong" invalid /></Cell>
      </div>
    </Section>

    <Section id="select" title="VSelect" base="PrimeVue Select" adds="Custom wrapper (Select.ts). With 8 or more options it turns the filter on, focuses the filter, highlights the match, shows an N of M count and resets the filter on hide.">
      <p class="row-title">Closed states</p>
      <div class="row">
        <Cell class="field" label="default"><VSelect v-model="shortValue" :options="SHORT" option-label="label" option-value="value" /></Cell>
        <Cell class="field" label="hover"><VSelect v-model="shortValue" :options="SHORT" option-label="label" option-value="value" class="pseudo-hover" /></Cell>
        <Cell class="field" label="focus"><VSelect v-model="shortValue" :options="SHORT" option-label="label" option-value="value" class="p-focus" /></Cell>
        <Cell class="field" label="placeholder"><VSelect v-model="emptyValue" :options="SHORT" option-label="label" option-value="value" placeholder="Choose" /></Cell>
        <Cell class="field" label="disabled"><VSelect :model-value="shortValue" :options="SHORT" option-label="label" option-value="value" disabled /></Cell>
        <Cell class="field" label="invalid"><VSelect v-model="emptyValue" :options="SHORT" option-label="label" option-value="value" placeholder="Choose" invalid /></Cell>
        <Cell class="field" label="loading"><VSelect v-model="emptyValue" :options="SHORT" option-label="label" option-value="value" placeholder="Loading" loading /></Cell>
        <Cell class="field" label="small"><VSelect v-model="shortValue" :options="SHORT" option-label="label" option-value="value" size="small" /></Cell>
      </div>
      <p class="row-title">Open</p>
      <div class="row">
        <Cell class="field open" label="short list, no filter">
          <VSelect v-model="shortValue" :options="SHORT" option-label="label" option-value="value" append-to="self" data-pin="select" />
        </Cell>
        <Cell class="field open tall" label="long list, filter with query" data-pin-cell="long">
          <VSelect v-model="longValue" :options="LONG" option-label="label" option-value="value" append-to="self" data-pin="select" />
        </Cell>
      </div>
    </Section>

    <Section id="multiselect" title="VMultiSelect" base="PrimeVue MultiSelect" adds="Re-export. Same overlay, option and filter tokens as Select.">
      <div class="row">
        <Cell class="field" label="default"><VMultiSelect v-model="multi" :options="LONG" option-label="label" option-value="value" display="chip" placeholder="Choose" /></Cell>
        <Cell class="field" label="disabled"><VMultiSelect :model-value="multi" :options="LONG" option-label="label" option-value="value" disabled /></Cell>
        <Cell class="field" label="invalid"><VMultiSelect :model-value="[]" :options="LONG" option-label="label" option-value="value" placeholder="Choose" invalid /></Cell>
        <Cell class="field open tall" label="open with filter">
          <VMultiSelect v-model="multi" :options="LONG" option-label="label" option-value="value" filter append-to="self" data-pin="multiselect" />
        </Cell>
      </div>
    </Section>

    <Section id="toggle" title="VToggleSwitch" base="PrimeVue ToggleSwitch" adds="Re-export. The preset sets the slide duration from the motion tokens.">
      <div class="row">
        <Cell label="on"><VToggleSwitch v-model="toggleOn" /></Cell>
        <Cell label="off"><VToggleSwitch v-model="toggleOff" /></Cell>
        <Cell label="hover"><VToggleSwitch :model-value="false" class="pseudo-hover" /></Cell>
        <Cell label="focus"><VToggleSwitch :model-value="true" class="p-focus" /></Cell>
        <Cell label="disabled on"><VToggleSwitch :model-value="true" disabled /></Cell>
        <Cell label="disabled off"><VToggleSwitch :model-value="false" disabled /></Cell>
        <Cell label="invalid"><VToggleSwitch :model-value="false" invalid /></Cell>
      </div>
    </Section>

    <Section id="chips" title="VChipGroup" base="custom (no PrimeVue base)" adds="Radio group of chips with arrow-key navigation, edge fade when it scrolls and an optional count badge. Styled only from tokens.">
      <div class="row">
        <Cell label="default, 'merged' is empty"><VChipGroup v-model="chip" :options="chipOptions" label="Filter" test-id="gallery-chip" /></Cell>
        <Cell label="narrow, scrolls with edge fade">
          <div style="width: 180px"><VChipGroup v-model="chip" :options="chipOptions" label="Filter narrow" test-id="gallery-chip-narrow" /></div>
        </Cell>
      </div>
    </Section>

    <Section id="tag" title="VTag" base="PrimeVue Tag" adds="Re-export. Not used in the app yet.">
      <div class="row">
        <Cell v-for="severity in TAG_SEVERITIES" :key="severity" :label="severity"><VTag :value="severity" :severity="severity" /></Cell>
        <Cell label="rounded"><VTag value="rounded" severity="info" rounded /></Cell>
      </div>
    </Section>

    <Section id="message" title="VMessage" base="PrimeVue Message" adds="Re-export. The app uses it for inline errors and warnings with size small.">
      <div class="stack" style="max-width: 560px">
        <VMessage v-for="severity in MESSAGE_SEVERITIES" :key="severity" :severity="severity" size="small">Message with severity {{ severity }}.</VMessage>
        <VMessage severity="error" closable>Closable message, default size.</VMessage>
      </div>
    </Section>

    <Section id="dialog" title="VDialog · DialogActions" base="PrimeVue Dialog" adds="VDialog is a re-export. DialogActions (apps/web/src/shared/ui) is imported unchanged: cancel, optional middle slot and confirm, small on desktop, danger and busy states.">
      <div class="row">
        <Cell label="standard dialog">
          <div class="frame">
            <VDialog visible modal append-to="self" :draggable="false" :close-on-escape="false" header="Move agent" :style="{ width: '24rem', maxWidth: '95%' }">
              <p class="frame-body">The agent keeps its conversation and continues in the target worktree.</p>
              <VSelect :model-value="shortValue" :options="SHORT" option-label="label" option-value="value" class="field" />
              <DialogActions confirm-label="Move" />
            </VDialog>
          </div>
        </Cell>
        <Cell label="danger dialog">
          <div class="frame">
            <VDialog visible modal append-to="self" :draggable="false" :close-on-escape="false" header="Delete session" :style="{ width: '24rem', maxWidth: '95%' }">
              <p class="frame-body">The session and its transcript are removed. This cannot be undone.</p>
              <DialogActions confirm-label="Delete" danger />
            </VDialog>
          </div>
        </Cell>
        <Cell label="live modal">
          <VButton label="Open modal dialog" severity="secondary" data-testid="gallery-open-dialog" @click="dialogOpen = true" />
          <VDialog v-model:visible="dialogOpen" modal dismissable-mask header="Unsaved changes" :style="{ width: '480px', maxWidth: '95vw' }" :draggable="false">
            <p class="frame-body">Save the changes before closing?</p>
            <DialogActions confirm-label="Save" focus="cancel" @cancel="dialogOpen = false" @confirm="dialogOpen = false">
              <template #middle="{ size }"><VButton label="Discard" :size="size" severity="danger" text @click="dialogOpen = false" /></template>
            </DialogActions>
          </VDialog>
        </Cell>
      </div>
      <p class="row-title">DialogActions variants</p>
      <div class="row">
        <Cell label="single (close)"><DialogActions /></Cell>
        <Cell label="cancel and confirm"><DialogActions confirm-label="Save" /></Cell>
        <Cell label="danger"><DialogActions confirm-label="Delete" danger /></Cell>
        <Cell label="busy"><DialogActions confirm-label="Saving" busy /></Cell>
        <Cell label="disabled"><DialogActions confirm-label="Save" disabled /></Cell>
      </div>
    </Section>

    <Section id="popover" title="VPopover · ConfirmPopover" base="PrimeVue Popover" adds="VPopover is a re-export. ConfirmPopover (apps/web/src/shared/ui) adds a question, a cancel button focused on show and a danger confirm.">
      <div class="row">
        <Cell class="open" label="popover">
          <VButton ref="popoverAnchor" label="Popover trigger" severity="secondary" size="small" @click="popover?.toggle($event)" />
          <VPopover ref="popover">
            <div class="pop-body">
              <strong>Share session</strong>
              <span>Anyone with the link can read the transcript.</span>
              <VInputText model-value="https://example.test/share/abc" size="small" />
            </div>
          </VPopover>
        </Cell>
        <Cell class="open" label="confirm popover" style="margin-left: 320px">
          <VButton ref="confirmAnchor" label="Delete" severity="danger" size="small" text @click="confirm?.show(($event.currentTarget as HTMLElement | null) ?? null)" />
          <ConfirmPopover ref="confirm" question="Delete this checkpoint?" confirm-label="Delete" />
        </Cell>
      </div>
    </Section>

    <Section id="menu" title="VContextMenu" base="PrimeVue ContextMenu" adds="Re-export. Items and focus colors come from the navigation tokens.">
      <div class="row">
        <Cell class="open" label="menu, right-click the box to reopen">
          <div ref="menuAnchor" class="vr-code" tabindex="0" @contextmenu.prevent="menu?.show($event)">right-click here</div>
          <VContextMenu ref="menu" :model="menuItems" />
        </Cell>
      </div>
    </Section>

    <Section id="candidates" title="Not in the kit" base="PrimeVue, used directly here" adds="Candidates the app now hand-rolls or does without. They render with the same preset and have no kit wrapper yet." candidate>
      <p class="row-title">Tooltip (directive). The app uses native title attributes</p>
      <div class="row">
        <Cell label="hover to show">
          <span ref="tooltipAnchor"><VButton v-tooltip.bottom="'Archive this session'" label="Archive" severity="secondary" size="small" /></span>
        </Cell>
      </div>
      <p class="row-title">Tabs. The app hand-rolls role=tab buttons in ProjectTree and ChangeRequestView</p>
      <div class="row">
        <Tabs v-model:value="tab">
          <TabList>
            <Tab value="overview">Overview</Tab>
            <Tab value="changes">Changes</Tab>
            <Tab value="pipeline">Pipeline</Tab>
          </TabList>
          <TabPanels>
            <TabPanel value="overview"><p class="frame-body">Overview panel</p></TabPanel>
            <TabPanel value="changes"><p class="frame-body">Changes panel</p></TabPanel>
            <TabPanel value="pipeline"><p class="frame-body">Pipeline panel</p></TabPanel>
          </TabPanels>
        </Tabs>
      </div>
      <p class="row-title">SelectButton and ToggleButton. The app hand-rolls aria-pressed buttons</p>
      <div class="row">
        <Cell label="segmented"><SelectButton v-model="segmented" :options="segmentedOptions" option-label="label" option-value="value" size="small" /></Cell>
        <Cell label="toggle on"><ToggleButton v-model="pressed" on-label="Wrap" off-label="Wrap" size="small" /></Cell>
        <Cell label="toggle off"><ToggleButton :model-value="false" on-label="Wrap" off-label="Wrap" size="small" /></Cell>
        <Cell label="toggle disabled"><ToggleButton :model-value="false" on-label="Wrap" off-label="Wrap" size="small" disabled /></Cell>
      </div>
      <p class="row-title">Checkbox. The app uses a native checkbox in UiInput</p>
      <div class="row">
        <Cell label="checked"><Checkbox v-model="checked" binary /></Cell>
        <Cell label="unchecked"><Checkbox :model-value="false" binary /></Cell>
        <Cell label="hover"><Checkbox :model-value="false" binary class="pseudo-hover" /></Cell>
        <Cell label="disabled"><Checkbox :model-value="true" binary disabled /></Cell>
        <Cell label="invalid"><Checkbox :model-value="false" binary invalid /></Cell>
      </div>
      <p class="row-title">ProgressBar and Badge. The app hand-rolls both</p>
      <div class="row">
        <Cell label="progress 40"><ProgressBar :value="40" style="width: 240px" /></Cell>
        <Cell label="badge"><Badge value="4" /></Cell>
        <Cell label="badge danger"><Badge value="2" severity="danger" /></Cell>
        <Cell label="badge success"><Badge value="9" severity="success" /></Cell>
      </div>
    </Section>
  </main>
</template>
