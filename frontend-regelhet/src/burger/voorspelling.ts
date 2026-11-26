// A slot on the application form for a panel that predicts the decision
// before the citizen applies. The citizen's entry fills it when its
// transport has one (the app in the browser, burgeromgeving/fsc4b/); without
// it the form has no such panel. The panel gets the article, the form and
// what the citizen filled in so far, and sends nothing anywhere.
import type { Component, InjectionKey } from 'vue';

export const VOORSPELLING: InjectionKey<Component> = Symbol('voorspelling');
