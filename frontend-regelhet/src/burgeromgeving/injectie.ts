// The transport of this build, provided once by main.ts to the shell, so a
// test can mount the shell with another one.
import type { InjectionKey } from 'vue';
import type { Vervoer } from './vervoer';

export const VERVOER: InjectionKey<Vervoer> = Symbol('vervoer');
