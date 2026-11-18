export const SUPPORT_EMAIL = 'regelrecht@minbzk.nl';

// Shared so the main search-field (AppShell) and the search popover's list
// stay in sync — they are two entry points to the same law/regulation search.
export const SEARCH_PLACEHOLDER = 'Regelwerken zoeken';

// Accessible name (aria-label) for the same search entry points; kept as a
// concise descriptor alongside the fuller visible placeholder.
export const SEARCH_ACCESSIBLE_LABEL = 'Zoeken in regelwerken';

// Single strategy for how every pane renders while its content loads.
// false = show only the loading indicator; the pane's title and toolbars appear
//         once loaded (navigation like a back button stays available).
// true  = reveal the title and toolbars immediately during load, everywhere at
//         once. Panes that only know their title after the fetch need it wired
//         from the selection first (the name is already on the list one level
//         up), so flipping this is a per-pane opt-in for those.
// Every pane gates its chrome through `paneChromeVisible(loading)` so this one
// value flips them all together.
export const SHOW_PANE_CHROME_WHILE_LOADING = true;

export const paneChromeVisible = (loading: boolean): boolean => !loading || SHOW_PANE_CHROME_WHILE_LOADING;
