<script setup lang="ts">
import { computed } from 'vue';
import { renderStep, stepKeywords } from '../data/gherkinNl';
import { displayCell, emphasiseArguments, hasHeaderRow, isExpectation, keywordColumnWidth, stepMark, type StepResult } from '../data/scenarioSteps';
import type { Step } from '@regelrecht/frontend-shared/gherkin';
import { activeLocale, useI18n } from '../i18n/index.js';

// The steps of a scenario (or of the background) as Gherkin: in order, keyword
// first, data tables as tables. It shows what the file says and nothing it
// would take the engine to know; the only marks from a run are pass/fail on
// the expectations, a fail mark on whichever step failed, and its error.
const props = withDefaults(defineProps<{
  steps: Step[];
  // Run status per step, aligned with `steps`: `{ status, error }` or nothing.
  results?: (StepResult | null | undefined)[];
}>(), {
  results: () => [],
});

const { t } = useI18n();

const result = (index: number) => props.results[index] ?? null;

// Sized to the longest keyword of the language that is on, so it is the same
// in the background and in every scenario, and follows a language switch.
const keywordWidth = computed(() => {
  // stepKeywords() reads the locale outside Vue's reach; this makes the switch a dependency.
  void activeLocale.value;
  return keywordColumnWidth(stepKeywords());
});

const mark = (step: Step, index: number) => stepMark(step, result(index));

/** The supporting text as lines: engine errors carry line breaks worth keeping. */
function supportingLines(step: Step, index: number) {
  const text = supporting(step, index);
  return text ? String(text).split('\n') : [];
}

function supporting(step: Step, index: number) {
  const r = result(index);
  if (r?.error) return r.error;
  // The icon is decorative to a screen reader; a passed expectation says so in words.
  if (r?.status === 'pass' && isExpectation(step)) return t('scenario.passed');
  return renderStep(step).matched ? undefined : t('scenario.error.unknown_step');
}

// One track per column, sized to its content, plus a trailing filler track so
// the row dividers run to the edge of the box. The table scrolls sideways on
// its own when it is wider than the card.
function columns(table: string[][]) {
  const width = Math.max(...table.map((row) => row.length));
  return `repeat(${width}, max-content) 1fr`;
}
</script>

<template>
  <nldd-container gap="8">
    <template v-for="(step, i) in steps" :key="i">
      <!-- Mark column, keyword column, step text. The keyword column is as wide
           as the language's longest keyword, so the step texts line up the way
           a feature file does. -->
      <nldd-container layout="row" gap="8" vertical-alignment="top">
        <nldd-icon-cell v-if="mark(step, i)" :icon="mark(step, i)?.icon" :color="mark(step, i)?.color" size="16" vertical-alignment="top"></nldd-icon-cell>
        <nldd-spacer-cell v-else size="16"></nldd-spacer-cell>
        <nldd-text-cell :width="keywordWidth" vertical-alignment="top" color="accent" :text="`**${renderStep(step).keyword}**`"></nldd-text-cell>
        <!-- The supporting text goes through the slot, line by line: the
             attribute collapses the line breaks in an engine error. -->
        <nldd-text-cell vertical-alignment="top" :text="emphasiseArguments(renderStep(step).text)">
          <span v-if="supportingLines(step, i).length" slot="supporting-text"
            ><template v-for="(line, li) in supportingLines(step, i)" :key="li"><br v-if="li" />{{ line }}</template></span
          >
        </nldd-text-cell>
      </nldd-container>
      <!-- Indented to the step text by the same two columns as the step row:
           the mark and the keyword. The last cell may shrink, so a wide table
           scrolls inside itself instead of pushing the card wider. -->
      <nldd-container v-if="step.dataTable?.length" layout="row" gap="8">
        <nldd-spacer-cell size="16"></nldd-spacer-cell>
        <nldd-cell :width="keywordWidth"></nldd-cell>
        <nldd-cell width="full">
        <nldd-table :columns="columns(step.dataTable)" :accessible-label="t('scenario.table_for', { step: renderStep(step).text })">
          <nldd-table-row v-if="hasHeaderRow(step)" slot="header">
            <nldd-text-cell v-for="(cell, ci) in step.dataTable[0]" :key="ci" size="sm" :text="cell.trim()"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row v-for="(row, ri) in step.dataTable.slice(hasHeaderRow(step) ? 1 : 0)" :key="ri">
            <nldd-text-cell
              v-for="(cell, ci) in row"
              :key="ci"
              size="sm"
              :color="displayCell(cell).quiet ? 'secondary' : undefined"
              :text="displayCell(cell).text"
            ></nldd-text-cell>
          </nldd-table-row>
        </nldd-table>
        </nldd-cell>
      </nldd-container>
    </template>
  </nldd-container>
</template>
