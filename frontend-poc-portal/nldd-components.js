// Design-system components the portal's own two pages render, one entry point
// each. The package root would pull in all ~110 components; this list is
// generated from the nldd-* tags in the pages and checked on every build by
// script/check-nldd-imports.mjs, so a newly used component fails the build
// instead of silently never upgrading.
//
// The pages themselves are rendered in Rust (packages/poc-portal/src/pagina.rs)
// rather than by a bundler, so this file is the only place that says which
// components the bundle must carry. Keep it in step with that module — the
// `only_elements_that_exist_in_the_design_system_are_used` test there lists the
// same set.
//
// Regenerate: npm run nldd:imports -w poc-portal-assets
// Eerst: zet data-scheme voordat het ontwerpsysteem geladen wordt, anders
// staat de pagina eerst kort in de verkeerde stand.
import './thema.js';
import '@nldd/design-system/styles';
// nldd-shell is the required root and the element that carries min-height:100dvh;
// without it the page background stops where the content does.
import '@nldd/design-system/shell';
import '@nldd/design-system/banner';
import '@nldd/design-system/button';
import '@nldd/design-system/card';
import '@nldd/design-system/collection';
import '@nldd/design-system/container';
import '@nldd/design-system/form-field';
import '@nldd/design-system/hero';
// Icons are their own component: nldd-link's start-icon renders through it, so
// without this the link shows its text and silently no icon.
import '@nldd/design-system/icon';
import '@nldd/design-system/link';
// `menu` brengt nldd-menu-item mee; het pakket heeft daar geen eigen
// ingang voor.
import '@nldd/design-system/menu';
import '@nldd/design-system/page';
import '@nldd/design-system/password-field';
import '@nldd/design-system/rich-text';
import '@nldd/design-system/simple-section';
import '@nldd/design-system/spacer';
import '@nldd/design-system/tag';
import '@nldd/design-system/title';
