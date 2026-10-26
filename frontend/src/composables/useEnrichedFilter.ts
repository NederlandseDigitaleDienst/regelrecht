// The search popover's "only enriched" filter, kept outside the component so
// it survives the popover closing and reopening within a visit. Not persisted:
// a fresh visit searches every law, which is what someone new expects.

import { ref } from 'vue';

export const enrichedOnly = ref(false);
