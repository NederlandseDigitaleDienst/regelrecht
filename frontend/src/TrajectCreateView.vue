<script setup lang="ts">
import { ref, watchEffect } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { singleQuery } from './lib/routeParams';
import TrajectCreateForm from './components/TrajectCreateForm.vue';
import { useTrajectsStore } from './stores/trajects';
import { homeTarget } from './composables/useLastVisitedRoute';

// New-traject page, reached from the traject chooser. It uses the same shared
// form as the TrajectMenu sheet; after creating, you go straight into the
// editor (or the traject-scoped library) of the new traject, with a law passed
// along (query `law`/`article`) preselected.
//
// Top-level route (not an AppShell child): no app chrome. The page carries its
// own top title bar with a back button to the traject chooser.
const route = useRoute();
const router = useRouter();
const { createTraject } = useTrajectsStore();

const formEl = ref<InstanceType<typeof TrajectCreateForm> | null>(null);
const createBusy = ref(false);
const createError = ref<string | null>(null);

watchEffect(() => {
  document.title = 'Nieuw traject · RegelRecht';
});

// Terug naar de chooser, met behoud van sectie + meegekregen wet.
function goBack() {
  router.push({ name: 'trajecten', query: route.query });
}

async function submitCreate() {
  createError.value = null;
  // The form is rendered unconditionally and fires the submit that calls this,
  // so the template ref is set.
  const { payload, error } = formEl.value!.buildPayload();
  if (error) {
    createError.value = error;
    return;
  }
  createBusy.value = true;
  try {
    // buildPayload returns a payload whenever it returns no error.
    const created = await createTraject(payload!);
    // Mirror the TrajectMenu guard: the backend's `create` handler always
    // calls `fill_ref()`, but a half-filled TrajectSummary would otherwise
    // navigate to `/editor/undefined/...` and silently no-op.
    if (!created.ref) {
      console.warn('TrajectCreateView: created traject has no ref', created);
      createError.value =
        'Traject aangemaakt maar kon niet worden geopend. Ga terug en selecteer het.';
      return;
    }
    const lawId = singleQuery(route.query.law) || undefined;
    const articleNumber = singleQuery(route.query.article) || undefined;
    await router.push(
      route.query.sectie === 'library'
        ? homeTarget({ trajectRef: created.ref, lawId, articleNumber })
        : { name: 'editor-traject', params: { trajectRef: created.ref, lawId, articleNumber } },
    );
  } catch (e) {
    // Same property read as before; createTraject rejects with a RequestError.
    createError.value = (e as { message?: string }).message || 'Aanmaken mislukt';
  } finally {
    createBusy.value = false;
  }
}
</script>

<template>
  <nldd-shell>
    <nldd-page sticky-header>
      <nldd-top-title-bar
        slot="header"
        text="Nieuw traject"
        :back-text="createBusy ? undefined : 'Trajecten'"
        collapse-anchor="nieuw-traject-titel"
        @back="goBack"
      ></nldd-top-title-bar>

      <nldd-simple-section width="800px">
        <nldd-title id="nieuw-traject-titel" size="display-3"><h3>Nieuw traject</h3></nldd-title>
        <nldd-spacer size="16"></nldd-spacer>
        <TrajectCreateForm ref="formEl"
          :busy="createBusy"
          :error="createError"
          @submit="submitCreate"
        />
      </nldd-simple-section>

      <nldd-page-footer slot="footer"></nldd-page-footer>
    </nldd-page>
  </nldd-shell>
</template>
