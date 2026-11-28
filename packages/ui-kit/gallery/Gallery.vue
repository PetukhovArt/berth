<script setup lang="ts">
import { nextTick, onMounted, ref } from 'vue';
import { DARK_CLASS, VBadge, VButton, VCheckbox, VChipGroup, VContextMenu, VDialog, VInputText, VListbox, VMenu, VMessage, VMiddleTruncate, VMultiSelect, VPopover, VProgressBar, VRowButton, VSelect, VSelectButton, VTabs, VTag, VTextarea, VToggleButton, VToggleSwitch, vTooltip } from '../src';
import { Folder, FolderOpen, GitBranch, Inbox, MessageSquare, SquareTerminal } from '../../../apps/web/src/shared/ui/icons';
import DialogActions from '../../../apps/web/src/shared/ui/DialogActions.vue';
import ConfirmPopover from '../../../apps/web/src/shared/ui/ConfirmPopover.vue';
import Section from './Section.vue';
import Cell from './Cell.vue';

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
const tabOptions = [
  { value: 'overview', label: 'Overview' },
  { value: 'changes', label: 'Changes', count: 12, tone: 'info' as const, title: 'Changed files' },
  { value: 'pipeline', label: 'Pipeline', count: 2, tone: 'danger' as const },
];
const tabOptionsDisabled = [
  { value: 'overview', label: 'Overview' },
  { value: 'changes', label: 'Changes' },
  { value: 'pipeline', label: 'Pipeline', disabled: true },
];
const segmented = ref('split');
const segmentedOptions = [
  { label: 'Code', value: 'code', title: 'Source only' },
  { label: 'Split', value: 'split' },
  { label: 'Preview', value: 'preview' },
];
const segmentedDisabled = [...segmentedOptions, { label: 'Diff', value: 'diff', disabled: true }];
const multiSegment = ref(['code']);
const unchecked = ref(false);
const BADGE_SEVERITIES = ['secondary', 'success', 'info', 'warn', 'danger', 'contrast'] as const;
const vmenu = ref<InstanceType<typeof VMenu>>();
const vmenuAnchor = ref<InstanceType<typeof VButton>>();
const vmenuItems = [{ label: 'Rename', testid: 'gallery-menu-rename' }, { label: 'Move to worktree' }, { separator: true }, { label: 'Delete', danger: true }];
const pressed = ref(true);
const checked = ref(true);
const ROVING = ['claude-main', 'codex-review', 'cursor-spike', 'claude-docs'];
const roving = ref('codex-review');
const NAV = [
  { label: 'Inbox', icon: Inbox },
  { label: 'Sessions', icon: SquareTerminal },
  { label: 'Channels', icon: MessageSquare },
];
const nav = ref('Sessions');

function rovingKey(event: KeyboardEvent) {
  const delta = event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : 0;
  if (!delta) return;
  event.preventDefault();
  const next = ROVING[(ROVING.indexOf(roving.value) + delta + ROVING.length) % ROVING.length];
  if (!next) return;
  roving.value = next;
  nextTick(() => document.querySelector<HTMLElement>(`[data-testid="gallery-roving-${next}"]`)?.focus());
}
const dialogOpen = ref(false);

const menuItems = [{ label: 'Rename', command: () => undefined }, { label: 'Move to worktree', command: () => undefined }, { separator: true }, { label: 'Close', command: () => undefined }];

const popoverOpen = ref(false);
const popover = ref<InstanceType<typeof VPopover>>();
const popoverAnchor = ref<InstanceType<typeof VButton>>();
const confirm = ref<InstanceType<typeof ConfirmPopover>>();
const confirmAnchor = ref<InstanceType<typeof VButton>>();
const menu = ref<InstanceType<typeof VContextMenu>>();
const menuAnchor = ref<HTMLElement>();

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
  if (vmenuAnchor.value) vmenu.value?.show({ currentTarget: vmenuAnchor.value.$el } as unknown as Event);
}

function pinTooltip() {
  document.querySelector('[data-pin="tooltip"]')?.dispatchEvent(new MouseEvent('mouseenter'));
}

function reopen() {
  setTimeout(() => {
    openOverlays();
    pinTooltip();
  }, 80);
}

onMounted(async () => {
  await nextTick();
  setTimeout(() => {
    openOverlays();
    setTimeout(() => {
      window.scrollTo(0, 0);
      setTimeout(pinTooltip, 100);
    }, 700);
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
      <a href="#tooltip">vTooltip</a>
      <a href="#tabs">VTabs</a>
      <a href="#segmented">VSelectButton / VToggleButton</a>
      <a href="#checkbox">VCheckbox</a>
      <a href="#progress">VProgressBar</a>
      <a href="#badge">VBadge</a>
      <a href="#vmenu">VMenu</a>
      <a href="#listbox">VListbox</a>
      <a href="#rowbutton">VRowButton</a>
      <a href="#middletruncate">VMiddleTruncate</a>
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
        <Cell label="beside controls (vr-tag-control)"><VTag value="label" severity="secondary" rounded class="vr-tag-control" /></Cell>
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
          <VButton ref="popoverAnchor" v-tooltip="'Opens the popover'" :aria-expanded="popoverOpen" aria-haspopup="dialog" label="Popover trigger" severity="secondary" size="small" @click="popover?.toggle($event)" />
          <VPopover ref="popover" @show="popoverOpen = true" @hide="popoverOpen = false">
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

    <Section id="tooltip" title="vTooltip" base="PrimeVue Tooltip" adds="Directive wrapper (Tooltip.ts). Shows after 400 ms, opens below unless a position modifier is given, and names an icon-only element with aria-label when it has no accessible name. Replaces the native title attribute.">
      <div class="row">
        <Cell label="hover to show (pinned open)">
          <VButton data-pin="tooltip" v-tooltip="'Archive this session'" label="Archive" severity="secondary" size="small" />
        </Cell>
        <Cell label="icon-only, named by the tooltip"><VButton v-tooltip="'More actions'" label="⋯" size="small" text severity="secondary" /></Cell>
        <Cell label="top"><VButton v-tooltip.top="'Opens above'" label="Top" size="small" severity="secondary" /></Cell>
        <Cell label="right"><VButton v-tooltip.right="'Opens on the right'" label="Right" size="small" severity="secondary" /></Cell>
        <Cell label="disabled (empty value)"><VButton v-tooltip="''" label="No tooltip" size="small" severity="secondary" /></Cell>
      </div>
    </Section>

    <Section id="tabs" title="VTabs" base="PrimeVue Tabs" adds="Wrapper (Tabs.vue). Takes an options array, adds a tablist label, a test id per tab, a count badge with a tone, a tooltip per tab and lazy panels from the panel slot. The preset draws the active tab with an underline.">
      <div class="row">
        <Cell label="header only, with counts and tones">
          <VTabs v-model="tab" :options="tabOptions" label="Request" test-id="gallery-tab" />
        </Cell>
        <Cell label="with panels, one disabled">
          <VTabs v-model="tab" :options="tabOptionsDisabled" label="Request panels" test-id="gallery-tab-panels">
            <template #panel="{ option }"><p class="frame-body">{{ option.label }} panel</p></template>
          </VTabs>
        </Cell>
      </div>
    </Section>

    <Section id="segmented" title="VSelectButton · VToggleButton" base="PrimeVue SelectButton · ToggleButton" adds="VSelectButton is a wrapper (SelectButton.vue). It takes an options array, a group label, a per-option test id and tooltip, and keeps one option selected unless allow-empty is set. VToggleButton is a re-export.">
      <div class="row">
        <Cell label="single"><VSelectButton v-model="segmented" :options="segmentedOptions" label="View" /></Cell>
        <Cell label="multiple (chips)"><VSelectButton v-model="multiSegment" :options="segmentedOptions" label="Layers" multiple /></Cell>
        <Cell label="with disabled option"><VSelectButton v-model="segmented" :options="segmentedDisabled" label="View with disabled" /></Cell>
        <Cell label="toggle on"><VToggleButton v-model="pressed" on-label="Wrap" off-label="Wrap" size="small" /></Cell>
        <Cell label="toggle off"><VToggleButton :model-value="false" on-label="Wrap" off-label="Wrap" size="small" /></Cell>
        <Cell label="toggle disabled"><VToggleButton :model-value="false" on-label="Wrap" off-label="Wrap" size="small" disabled /></Cell>
      </div>
    </Section>

    <Section id="checkbox" title="VCheckbox" base="PrimeVue Checkbox" adds="Wrapper (Checkbox.vue). Binary and small by default, links a label to the box so a click on the text toggles it.">
      <div class="row">
        <Cell label="checked"><VCheckbox v-model="checked" label="Include diffs" /></Cell>
        <Cell label="unchecked"><VCheckbox v-model="unchecked" label="Delete branch" /></Cell>
        <Cell label="no label"><VCheckbox v-model="checked" aria-label="Select row" /></Cell>
        <Cell label="hover"><VCheckbox :model-value="false" label="Hover" class="pseudo-hover" /></Cell>
        <Cell label="disabled"><VCheckbox :model-value="true" label="Disabled" disabled /></Cell>
        <Cell label="invalid"><VCheckbox :model-value="false" label="Invalid" invalid /></Cell>
      </div>
    </Section>

    <Section id="progress" title="VProgressBar" base="PrimeVue ProgressBar" adds="Wrapper (ProgressBar.vue). Requires an accessible label, hides the percentage by default, turns indeterminate when the value is null and takes a severity colour from the status tokens.">
      <div class="row">
        <Cell label="40"><VProgressBar :value="40" label="Progress" style="width: 220px" /></Cell>
        <Cell label="success 100"><VProgressBar :value="100" severity="success" label="Done" style="width: 220px" /></Cell>
        <Cell label="warn 70"><VProgressBar :value="70" severity="warn" label="Usage" style="width: 220px" /></Cell>
        <Cell label="danger 95"><VProgressBar :value="95" severity="danger" label="Limit" style="width: 220px" /></Cell>
        <Cell label="indeterminate"><VProgressBar label="Loading" style="width: 220px" /></Cell>
      </div>
    </Section>

    <Section id="badge" title="VBadge" base="PrimeVue Badge" adds="Wrapper (Badge.vue). Caps a number at max (99+), and renders a dot when dot is set or the value is empty.">
      <div class="row">
        <Cell v-for="severity in BADGE_SEVERITIES" :key="severity" :label="severity"><VBadge :value="4" :severity="severity" /></Cell>
        <Cell label="capped"><VBadge :value="340" /></Cell>
        <Cell label="small"><VBadge :value="7" size="small" /></Cell>
        <Cell label="large"><VBadge :value="7" size="large" /></Cell>
        <Cell label="dot"><VBadge dot severity="danger" /></Cell>
      </div>
    </Section>

    <Section id="vmenu" title="VMenu" base="PrimeVue Menu" adds="Wrapper (Menu.vue). Popup by default, items take a test id and a danger flag, and toggle, show and hide are forwarded.">
      <div class="row">
        <Cell class="open" label="popup menu, pinned open">
          <VButton ref="vmenuAnchor" label="Actions" size="small" severity="secondary" aria-haspopup="menu" @click="vmenu?.toggle($event)" />
          <VMenu ref="vmenu" :model="vmenuItems" label="Actions" />
        </Cell>
      </div>
    </Section>

    <Section id="listbox" title="VListbox" base="PrimeVue Listbox" adds="Wrapper (Listbox.vue). Takes label and value options, turns the filter on from 8 options and highlights the match like VSelect, and keeps one option selected.">
      <div class="row">
        <Cell class="field" label="short, single"><VListbox v-model="shortValue" :options="SHORT" label="Agent" /></Cell>
        <Cell class="field" label="long, filter"><VListbox v-model="longValue" :options="LONG" label="City" /></Cell>
        <Cell class="field" label="multiple"><VListbox v-model="multi" :options="LONG.slice(0, 5)" multiple label="Cities" /></Cell>
        <Cell class="field" label="disabled"><VListbox :model-value="shortValue" :options="SHORT" label="Agent disabled" disabled /></Cell>
      </div>
    </Section>

    <Section id="rowbutton" title="VRowButton" base="native button (custom, no PrimeVue base)" adds="Wrapper (RowButton.vue). Full-width interactive row with leading, default and trailing slots, an active state that also sets aria-current, compact and regular density, a depth indent for tree rows, a stacked layout for bottom navigation and a focus-visible ring. Native Enter and Space; tabindex, role and aria-* pass through for roving focus.">
      <p class="row-title">States</p>
      <div class="row">
        <Cell class="field" label="default"><VRowButton><template #leading><GitBranch :size="14" /></template>feature/search-filter</VRowButton></Cell>
        <Cell class="field" label="hover"><VRowButton class="pseudo-hover"><template #leading><GitBranch :size="14" /></template>feature/search-filter</VRowButton></Cell>
        <Cell class="field" label="focus-visible"><VRowButton class="pseudo-focus"><template #leading><GitBranch :size="14" /></template>feature/search-filter</VRowButton></Cell>
        <Cell class="field" label="active"><VRowButton active><template #leading><GitBranch :size="14" /></template>feature/search-filter</VRowButton></Cell>
        <Cell class="field" label="disabled"><VRowButton disabled><template #leading><GitBranch :size="14" /></template>feature/search-filter</VRowButton></Cell>
      </div>
      <p class="row-title">Slots and ellipsis</p>
      <div class="row">
        <Cell class="field" label="leading, main, trailing">
          <VRowButton><template #leading><MessageSquare :size="14" /></template>release-notes<template #trailing><span class="rb-meta">3m</span></template></VRowButton>
        </Cell>
        <Cell class="field" label="long text ellipsizes, trailing stays">
          <VRowButton data-testid="gallery-row-long"><template #leading><MessageSquare :size="14" /></template>a very long channel name that cannot fit into the row width<template #trailing><span class="rb-meta">12h</span></template></VRowButton>
        </Cell>
        <Cell class="field" label="two lines, align start">
          <VRowButton align="start">
            <template #leading><Inbox :size="14" /></template>
            <span class="rb-body"><span class="rb-line"><span class="rb-name">Fix login redirect</span><span class="rb-meta">5m</span></span><span class="rb-meta">Needs your answer · berth</span></span>
          </VRowButton>
        </Cell>
      </div>
      <p class="row-title">Density and tree depth</p>
      <div class="row">
        <Cell class="field" label="regular"><VRowButton><template #leading><SquareTerminal :size="14" /></template>Regular row</VRowButton></Cell>
        <Cell class="field" label="compact tree, depth 0 / 1 / 2">
          <div class="stack">
            <VRowButton density="compact"><template #leading><FolderOpen :size="14" /></template>src<template #trailing><span class="rb-meta">+12 -3</span></template></VRowButton>
            <VRowButton density="compact" :depth="1"><template #leading><Folder :size="14" /></template>widgets<template #trailing><span class="rb-meta">+4 -0</span></template></VRowButton>
            <VRowButton density="compact" :depth="2" active><template #leading><SquareTerminal :size="14" /></template>RowButton.vue<template #trailing><span class="rb-meta">+90 -0</span></template></VRowButton>
          </div>
        </Cell>
      </div>
      <p class="row-title">Roving focus list (Tab enters, arrows move, Enter and Space activate)</p>
      <div class="row">
        <Cell class="field" label="role listbox, aria-selected and tabindex passed through">
          <div role="listbox" aria-label="Sessions" class="stack" @keydown="rovingKey">
            <VRowButton v-for="item in ROVING" :key="item" role="option" density="compact" :active="roving === item" :aria-selected="roving === item" :tabindex="roving === item ? 0 : -1" :data-testid="`gallery-roving-${item}`" @click="roving = item">
              <template #leading><SquareTerminal :size="14" /></template>{{ item }}
            </VRowButton>
          </div>
        </Cell>
      </div>
      <p class="row-title">Stacked (phone bottom navigation)</p>
      <div class="row">
        <Cell label="stacked, second active">
          <div class="rb-nav">
            <VRowButton v-for="item in NAV" :key="item.label" stacked :active="nav === item.label" class="rb-nav-item" @click="nav = item.label">
              <template #leading><component :is="item.icon" :size="20" /></template>{{ item.label }}
            </VRowButton>
          </div>
        </Cell>
      </div>
    </Section>

    <Section id="middletruncate" title="VMiddleTruncate" base="native span (custom, no PrimeVue base)" adds="Wrapper (MiddleTruncate.vue). Cuts a long name in the middle with an ellipsis so the tail stays visible. A shared ResizeObserver measures each name once per resize with canvas text metrics, so there is no per-frame layout work. A name that fits is plain text with a CSS end ellipsis and a short one never splits. The full text stays in the DOM for screen readers, text search and the tooltip of the row that holds it.">
      <p class="row-title">Widths</p>
      <div class="row">
        <Cell class="field" label="fits"><VMiddleTruncate text="feat/search-filter" /></Cell>
        <Cell class="field" label="branch, 200px"><VMiddleTruncate text="feat/very-long-branch-name" data-testid="gallery-mt-branch" /></Cell>
        <Cell class="field" label="same prefix"><VMiddleTruncate text="feat/very-long-branch-name-v2" /></Cell>
        <Cell class="field" label="CJK"><VMiddleTruncate text="修复登录重定向问题并补充会话列表的长名称显示测试" /></Cell>
        <Cell class="field" label="short, end ellipsis"><VMiddleTruncate text="release-notes" /></Cell>
      </div>
      <p class="row-title">In a row</p>
      <div class="row">
        <Cell class="field" label="leading and trailing stay">
          <VRowButton><template #leading><GitBranch :size="14" /></template><VMiddleTruncate text="feat/very-long-branch-name-for-the-tree" /><template #trailing><span class="rb-meta">3m</span></template></VRowButton>
        </Cell>
      </div>
    </Section>
  </main>
</template>
