import { computed } from 'vue';

/**
 * Shared filter helpers for the data/grouped tables.
 *
 * A search/filter is "active" when any filter holds a non-empty value. This
 * lets the caller tell an *empty* state (no data at all → hide the toolbar)
 * apart from a *no-results* state (filters excluded everything → keep the
 * toolbar so the user can clear or change the search).
 *
 * @param getFilters - returns the current filters object
 * @param emit - component emit
 */
export function useTableFilters(
  getFilters: () => Record<string, unknown> | null | undefined,
  emit: (event: 'filter-change', key: string, value: string) => void,
) {
  const hasActiveFilters = computed(() =>
    Object.values(getFilters() || {}).some((v) => v !== '' && v != null),
  );

  // Clear every active filter by emitting an empty value per key - the
  // parent's setFilter handler deletes a filter when given a falsy value.
  function clearFilters() {
    for (const key of Object.keys(getFilters() || {})) {
      emit('filter-change', key, '');
    }
  }

  return { hasActiveFilters, clearFilters };
}
