/// <reference types="vite/client" />

// The design system's stylesheet, imported for its side effect in main.ts.
// Its `exports` entry points at a .css file, which carries no type declaration.
declare module '@nldd/design-system/styles';
