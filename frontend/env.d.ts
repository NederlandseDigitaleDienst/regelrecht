/// <reference types="vite/client" />

// The design system's stylesheet, imported for its side effect in main.
// Its `exports` entry points at a .css file, which carries no type declaration.
declare module '@nldd/design-system/styles';

/** Law ids from public/favorites.json, injected at build time (vite.config.ts). */
declare const __SUGGESTED_LAW_IDS__: string[];
