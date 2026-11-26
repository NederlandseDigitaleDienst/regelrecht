<script setup lang="ts">
// What is behind the readable error messages (lib/userError), for whoever
// repairs, reports or debugs the problem. Mounted once in the shell and driven
// by the errors store. The code viewer brings its own copy button.
//
// An error the engine gave as records (lib/engineDiagnostics) lists its
// findings per article, each a row that opens the place it is about.
import { storeToRefs } from 'pinia';
import { useErrorsStore, type ReportedError } from '../stores/errors';
import { placeLabel, ruleText, type EngineDiagnostic } from '../lib/engineDiagnostics';

const store = useErrorsStore();
const { detailsOpen, detailsOf } = storeToRefs(store);

interface ArticleFindings {
  article: string;
  findings: EngineDiagnostic[];
}

/** The findings that name an article, grouped by it, in the engine's order. */
function byArticle(error: ReportedError): ArticleFindings[] {
  const groups: ArticleFindings[] = [];
  for (const finding of error.diagnostics ?? []) {
    if (!finding.article) continue;
    const group = groups.find((g) => g.article === finding.article);
    if (group) group.findings.push(finding);
    else groups.push({ article: finding.article, findings: [finding] });
  }
  return groups;
}

function open(error: ReportedError, finding: EngineDiagnostic): void {
  store.closeDetails();
  error.goTo?.(finding);
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet placement="right" width="560px" :open="detailsOpen" data-testid="error-details-sheet" @close="store.closeDetails">
      <nldd-page sticky-header>
        <nldd-top-title-bar slot="header" text="Technische details" dismiss-text="Sluit"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <template v-for="(error, index) in detailsOf" :key="index">
            <nldd-spacer v-if="index > 0" size="32"></nldd-spacer>
            <nldd-title size="display-5"><h2>{{ error.title }}</h2></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-rich-text>
              <p>{{ error.summary }}</p>
            </nldd-rich-text>

            <template v-for="group in byArticle(error)" :key="group.article">
              <nldd-spacer size="16"></nldd-spacer>
              <nldd-title size="display-6"><h3>Artikel {{ group.article }}</h3></nldd-title>
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-list appearance="box-tinted" :accessible-label="`Bevindingen in artikel ${group.article}`">
                <nldd-list-item
                  v-for="(finding, findingIndex) in group.findings"
                  :key="findingIndex"
                  size="md"
                  :button="error.goTo ? true : undefined"
                  data-testid="error-details-finding"
                  @click="open(error, finding)"
                >
                  <nldd-text-cell
                    :text="placeLabel(finding.place) ?? `Artikel ${group.article}`"
                    :supporting-text="ruleText(finding) ?? finding.message"
                  ></nldd-text-cell>
                  <nldd-icon-cell v-if="error.goTo" size="20" icon="chevron-right" disclosure></nldd-icon-cell>
                </nldd-list-item>
              </nldd-list>
            </template>

            <template v-if="error.details">
              <nldd-spacer size="16"></nldd-spacer>
              <nldd-rich-text>
                <p>Stuur deze tekst mee als je het probleem meldt.</p>
              </nldd-rich-text>
              <nldd-spacer size="8"></nldd-spacer>
              <!-- Keyed: the viewer reads its slot once, so new text needs a new viewer. -->
              <nldd-code-viewer :key="error.details" wrap data-testid="error-details-text">{{ error.details }}</nldd-code-viewer>
            </template>
          </template>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
