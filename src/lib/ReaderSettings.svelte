<script>
import { tick } from 'svelte';

// The reader's settings panel, docked under the reading area. Each box saves as soon as
// it holds a valid number, so closing the panel never loses a change. The layout choice
// applies as soon as it is picked.
let { open, padding, twoColumns, onChange, onLayout } = $props();

const SIDES = [
  { key: 'top', label: 'Top' },
  { key: 'right', label: 'Right' },
  { key: 'bottom', label: 'Bottom' },
  { key: 'left', label: 'Left' },
];

/** @type {Record<string, string>} */
let drafts = $state({});
/** @type {Record<string, HTMLInputElement>} */
let inputs = {};

// Values changed elsewhere (another window, or a clamp) show up in every box but the one being typed in.
$effect(() => {
  for (const { key } of SIDES) {
    if (document.activeElement !== inputs[key]) drafts[key] = String(padding[key]);
  }
});

function valid(/** @type {string} */ text) {
  return /^\d{1,4}$/.test(text.trim());
}

function onInput(/** @type {string} */ key) {
  const text = drafts[key];
  if (valid(text)) onChange({ ...padding, [key]: Number(text.trim()) });
}

function onBlur(/** @type {string} */ key) {
  drafts[key] = String(padding[key]);
}

// The panel stays in the page while closed, so it can glide open. Each time it opens, the
// first box takes the keyboard.
$effect(() => {
  if (open) tick().then(() => inputs.top?.focus());
});
</script>

<div class="panel">
  <h2>Padding</h2>
  <div class="boxes">
    {#each SIDES as { key, label }}
      <label>
        <span>{label}</span>
        <input
          class="zen-input"
          class:invalid={!valid(drafts[key] ?? '')}
          type="text"
          inputmode="numeric"
          spellcheck="false"
          bind:this={inputs[key]}
          bind:value={drafts[key]}
          oninput={() => onInput(key)}
          onblur={() => onBlur(key)}
        />
      </label>
    {/each}
  </div>
  <h2 class="second">Layout</h2>
  <div class="choices" role="radiogroup" aria-label="Layout">
    <button class="zen-btn choice" role="radio" aria-checked={!twoColumns} class:on={!twoColumns} onclick={() => onLayout(false)}>One column</button>
    <button class="zen-btn choice" role="radio" aria-checked={twoColumns} class:on={twoColumns} onclick={() => onLayout(true)}>Two columns</button>
  </div>
</div>

<style>
.panel {
  flex: none;
  border-top: 1px solid var(--zen-border);
  background: var(--zen-bg);
  padding: 14px 24px 20px;
}
h2 {
  margin: 0 0 12px;
  font-family: var(--zen-font-serif); font-weight: 300; font-size: 16px;
  transform: scaleY(1.15); transform-origin: left;
  color: var(--zen-ink);
}
.boxes { display: flex; gap: 16px; }
label { display: flex; flex-direction: column; gap: 4px; }
label span { font-size: 11px; color: var(--zen-text-muted); }
input {
  width: 64px; box-sizing: border-box; height: 30px; padding: 0 8px;
  font-family: var(--zen-font-sans); font-size: 13px; color: var(--zen-ink);
  background: var(--zen-panel);
  border: 1px solid var(--zen-border); border-radius: 2px;
}
input:focus { outline: none; border-color: var(--zen-ink); }
input.invalid { border-color: var(--zen-red); }
h2.second { margin-top: 16px; }
.choices { display: flex; gap: 8px; }
.choice {
  height: 30px; padding: 0 12px; border-radius: 2px;
  border: 1px solid var(--zen-border); background: var(--zen-panel);
  font-size: 13px; color: var(--zen-text-secondary);
}
.choice.on { border-color: var(--zen-ink); color: var(--zen-ink); }
</style>
