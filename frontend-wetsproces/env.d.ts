/// <reference types="vite/client" />

// The design system's stylesheet and its themes (the accent colours),
// imported for their side effect in start.ts and burgeromgeving/main.ts. Their
// `exports` entries point at .css files, which carry no type declaration.
declare module '@nldd/design-system/styles';
declare module '@nldd/design-system/themes';
