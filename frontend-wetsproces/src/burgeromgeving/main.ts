// The citizen's entry (burger.html): the citizen's screens on their own,
// with the transport this build carries (`#vervoer`, see vervoer.ts). Nothing
// here imports the back office; scheiding.test.ts holds that.
import '@nldd/design-system';
import '@nldd/design-system/styles';
import { createApp } from 'vue';
import { useColorScheme } from '@regelrecht/frontend-shared';
import { vervoer } from '#vervoer';
import { gebruikSessie } from '../gedeeld/sessie';
import BurgerShell from './BurgerShell.vue';
import { VERVOER } from './injectie';
import { maakBurgerRouter } from './router';

const router = maakBurgerRouter(vervoer);
gebruikSessie(vervoer.sessie((reden) => void router.push({ name: 'toegang', query: { reden } })));

useColorScheme();
createApp(BurgerShell).provide(VERVOER, vervoer).use(router).mount('#app');
