// Design-system components this app renders, one entry point each.
// The package root would pull in all ~110 components; this list is generated
// from the nldd-* tags in the source and checked on every build by
// script/check-nldd-imports.mjs, so a newly used component fails the build
// instead of silently never upgrading.
//
// A component this app only names in prose (markdown inline code) is absent
// here on purpose: such a name has to exist, not to be imported.
//
// Regenerate: npm run nldd:imports
import '@nldd/design-system/badge';
import '@nldd/design-system/button';
import '@nldd/design-system/button-bar';
import '@nldd/design-system/cell';
import '@nldd/design-system/icon-button';
import '@nldd/design-system/icon-cell';
import '@nldd/design-system/list';
import '@nldd/design-system/list-item';
import '@nldd/design-system/page';
import '@nldd/design-system/search-field';
import '@nldd/design-system/segmented-control';
import '@nldd/design-system/spacer-cell';
import '@nldd/design-system/tag';
import '@nldd/design-system/text-cell';
import '@nldd/design-system/toolbar';
