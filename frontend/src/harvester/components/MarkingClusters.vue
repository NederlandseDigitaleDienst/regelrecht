<script setup lang="ts">
import { computed } from 'vue';
import type { MarkingCluster } from '../composables/useMarkingClusters';
import { providerLabel } from '../../lib/enrichProviders';

// The backlog, read off the corpus: markings grouped by the change that would
// resolve them. One marking is an observation; the same change wanted by four
// articles across two laws is a pattern, and the pattern is what decides
// whether it is worth building.
//
// A sibling of the list rather than a separate view, because it is an
// aggregation over exactly the rows below it. Clicking a cluster filters that
// list, which is how a reader gets from "wanted in four places" to the four
// articles themselves.
//
// The visual order answers the question the screen exists for: what should we
// build, and in what order?
//
//   1. Reach, as a number, leading each row. It is what ranks the list, so it
//      reads first and scans down the left edge as a column of figures.
//   2. The change itself, at full weight, because that is what becomes work.
//   3. Where it bites (articles, laws) directly under it, quieter.
//   4. A badge for the kind of change, since `operation` and `model` are a
//      closed pair: two shapes to recognise, not two words to read.
//   5. Exceptions on the right, and only when true: the enricher hint, and
//      "beoordeeld". Anything that holds for most rows is left unsaid.
const props = withDefaults(
  defineProps<{
    clusters?: MarkingCluster[];
    loading?: boolean;
  }>(),
  {
    clusters: () => [],
    loading: false,
  },
);

defineEmits<{
  select: [cluster: MarkingCluster];
}>();

// A cluster of one is the interesting case, not the boring one. `resolved_by`
// is free text, so two agents asking for the same change in different words
// land in two clusters of one, and a reader has to be told that rather than
// left to infer it from a long tail.
const singletons = computed(() => props.clusters.filter((c) => c.markings === 1).length);

// A change only one provider ever asks for, while another ran over the same
// laws without complaint, points at the enricher rather than at the format.
// The two are indistinguishable from the marking alone, so the signal has to
// be visible here.
//
// It only says something when there was another provider to disagree. With one
// provider in the corpus every cluster has exactly one, and a hint that fires
// on all of them is not a signal, it is decoration.
function providerHint(cluster: MarkingCluster): string | null {
  const providers = cluster.providers || [];
  const only = providers[0];
  if (providers.length !== 1 || !only) return null;
  if ((cluster.providers_on_these_laws ?? 0) < 2) return null;
  return `alleen ${providerLabel(only)}`;
}

// Where the change bites. Articles before laws reads naturally, and the
// ranking already leans on laws: three articles inside one law may be that
// law's peculiarity, while three spread over three laws is the format.
function reachText(cluster: MarkingCluster): string {
  const laws = cluster.laws === 1 ? '1 wet' : `${cluster.laws} wetten`;
  const articles = cluster.articles === 1 ? '1 artikel' : `${cluster.articles} artikelen`;
  return `${articles} in ${laws}`;
}

// A closed pair, so a badge reads faster than a word: two shapes to recognise
// rather than two labels to parse.
const KIND: Record<string, { text: string; color: string }> = {
  operation: { text: 'bewerking', color: 'accent' },
  model: { text: 'formaat', color: 'neutral' },
};

function kind(cluster: MarkingCluster): { text: string; color: string } {
  return KIND[cluster.resolution] || { text: cluster.resolution, color: 'neutral' };
}
</script>

<template>
  <nldd-simple-section v-if="!loading && clusters.length">
    <!-- A card, like the panels on the overview: the backlog is a block with
         its own edge, not loose text above the table. `nldd-container` carries
         the padding, so nothing here needs custom CSS. -->
    <nldd-card>
      <nldd-container padding="16">
        <nldd-title size="display-5"><h2>Wat het formaat mist</h2></nldd-title>
        <nldd-spacer size="4" />
        <nldd-rich-text>
          <p>
            Markeringen gegroepeerd op de wijziging die ze oplost, de breedste
            eerst. Iets wat in één wet speelt kan die wet zijn; iets wat in
            drie wetten speelt is het formaat.
          </p>
        </nldd-rich-text>

        <nldd-spacer size="16" />

        <nldd-list appearance="simple">
          <nldd-list-item
            v-for="(cluster, i) in clusters"
            :key="`${cluster.resolution}-${cluster.resolved_by}-${i}`"
            button
            @click="$emit('select', cluster)"
          >
            <!-- The count leads: it is what ranks the list, and a column of
                 figures down the left edge is scannable in a way a sentence
                 is not. -->
            <nldd-text-cell
              :text="String(cluster.markings)"
              supporting-text="markeringen"
              width="fit-content"
              min-width="76px"
            />
            <nldd-spacer-cell size="12" />

            <!-- The change itself, at full weight: this is what becomes work.
                 Where it bites sits under it, quieter. -->
            <nldd-text-cell
              :text="cluster.resolved_by || 'niet benoemd'"
              :supporting-text="reachText(cluster)"
            />

            <nldd-spacer-cell />

            <!-- Exceptions only, and only when true. Anything that holds for
                 most rows would be noise here. -->
            <nldd-text-cell
              v-if="providerHint(cluster)"
              :text="providerHint(cluster)"
              supporting-text="mogelijk de enricher"
              width="fit-content"
              min-width="150px"
              color="warning"
              horizontal-alignment="right"
            />
            <nldd-cell v-if="cluster.all_accepted" width="fit-content">
              <nldd-badge color="success" size="sm" text="beoordeeld" />
            </nldd-cell>

            <nldd-spacer-cell size="12" />
            <nldd-cell width="fit-content">
              <nldd-badge :color="kind(cluster).color" size="sm" :text="kind(cluster).text" />
            </nldd-cell>
          </nldd-list-item>
        </nldd-list>

        <!-- Exact grouping on free text undercounts, and a reader has to be
             told rather than left to infer it from a tail of ones. -->
        <template v-if="singletons">
          <nldd-spacer size="16" />
          <nldd-inline-dialog
            variant="alert"
            :text="`${singletons} ${singletons === 1 ? 'cluster telt' : 'clusters tellen'} één markering`"
            supporting-text="Groeperen gebeurt op de letterlijke tekst van 'wat het oplost', dus dezelfde wens in andere woorden staat hier twee keer."
          />
        </template>
      </nldd-container>
    </nldd-card>

  </nldd-simple-section>
</template>
