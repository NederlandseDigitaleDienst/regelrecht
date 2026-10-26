import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { deckFor, escapeStops, usePresentation } from './usePresentation';
import type { DeckRoute, Presentation, PresentationDemo, SlideConfig } from './usePresentation';
import { adoptLocale } from '../i18n/index.js';

// De presentatie hangt één keydown-luisteraar aan het venster en houdt die
// vast zolang ze actief is. Dat is wat deze tests bewaken: zodra het scherm
// niet meer van de dia's is, mag ze spatie en de pijltjes niet meer afvangen.
//
// De module houdt zijn staat (ook de router) op moduleniveau en `init` schrijft
// alleen wat je meegeeft. Elke test zet daarom in `beforeEach` een verse router
// én dia's, en eindigt met `stop()`. Laat een test de router weg, dan erft hij
// stilzwijgend die van de vorige en bewijst hij niets.

/**
 * Een router-dubbel met alleen wat de presentatie ervan leest en gebruikt.
 * De tabbladen dragen een naam en sommige een optionele parameter, net als in
 * router.ts: `/regelwerken/:lawId?` houdt dezelfde naam als de presentator een wet
 * opent.
 */
const ROUTES: { name: string; path: string }[] = [
  { name: 'home', path: '/' },
  { name: 'wetten', path: '/regelwerken' },
  { name: 'simulatie', path: '/simulatie' },
  { name: 'zaaksysteem', path: '/zaaksysteem' },
];

/**
 * Onbekend pad geeft `undefined`, net als de echte router: die vangt zoiets op
 * met `/:pathMatch(.*)*`, en die route draagt geen naam.
 */
function nameFor(path: string): string | undefined {
  const head = `/${String(path).split('/')[1] ?? ''}`;
  return ROUTES.find((r) => r.path === head)?.name;
}

/**
 * Een route zoals de echte router hem teruggeeft, inclusief `meta.page`: het
 * dek vergelijkt daarop, omdat dezelfde pagina per taal een eigen routenaam
 * heeft (`wetten` en `wetten:en`) en `meta.page` is wat die twee delen.
 */
/** De Engelse slugs van de paden die deze tests aanraken. */
const EN_PATHS: Record<string, string | undefined> = { wetten: '/ruleworks', simulatie: '/simulation', zaaksysteem: '/cases' };

function routeFor(path: string): DeckRoute {
  // Een Engels pad hoort bij dezelfde pagina als zijn Nederlandse tegenhanger:
  // dat is precies wat `meta.page` uitdrukt, en waar het dek op vergelijkt.
  const enPage = Object.keys(EN_PATHS).find((page) => path === `/en${EN_PATHS[page]}`);
  if (enPage) return { path, name: `${enPage}:en`, meta: { page: enPage, locale: 'en' } };
  if (path === '/en') return { path, name: 'home:en', meta: { page: 'home', locale: 'en' } };
  const name = nameFor(path);
  return { path, name, meta: name ? { page: name, locale: 'nl' } : {} };
}

function fakeRouter(path = '/') {
  const currentRoute: { value: DeckRoute } = { value: routeFor(path) };
  return {
    currentRoute,
    /**
     * De echte router wordt zowel met een pad als met `{ name }` aangeroepen;
     * het dek doet dat laatste om een dia-pad naar de actieve taal te brengen.
     */
    resolve(to: string | { name: string }): DeckRoute {
      if (to && typeof to === 'object') {
        const en = String(to.name).endsWith(':en');
        const hit = ROUTES.find((r) => r.name === String(to.name).replace(/:en$/, ''));
        if (!hit) return { path: '/', name: undefined, meta: {} };
        if (!en) return routeFor(hit.path);
        const path = hit.path === '/' ? '/en' : `/en${EN_PATHS[hit.name] ?? hit.path}`;
        return { path, name: `${hit.name}:en`, meta: { page: hit.name, locale: 'en' } };
      }
      return routeFor(to);
    },
    push(to: string): Promise<unknown> {
      currentRoute.value = routeFor(to);
      return Promise.resolve();
    },
    /** Wat de presentator zelf doet: klikken en navigeren buiten de dia's om. */
    goTo(to: string): void {
      currentRoute.value = routeFor(to);
    },
  };
}

type FakeRouter = ReturnType<typeof fakeRouter>;

const SLIDES: [SlideConfig, SlideConfig, SlideConfig] = [
  { kind: 'title', title: 'Opening' },
  { kind: 'demo', title: 'De wetten', route: '/regelwerken' },
  { kind: 'closing', title: 'Slot' },
];

/** Stuur een keydown naar het venster en meld of iemand hem afving. */
function press(key: string): boolean {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
  document.body.dispatchEvent(event);
  return event.defaultPrevented;
}

/**
 * Een toetsaanslag uit een invoerveld van het ontwerpsysteem, zoals Chrome hem
 * aflevert: het event is geretarget naar de host (`e.target` is de custom
 * element) en de echte `<input>` staat alleen op `composedPath()`.
 *
 * Nagebouwd in plaats van nagespeeld, want happy-dom implementeert die
 * retargeting niet: daar blijft `e.target` gewoon de `<input>`, en dan zou deze
 * test groen staan zonder iets te bewijzen. In Chrome is gemeten dat
 * `e.target` `NLDD-TEXT-FIELD` is en `closest('input')` niets vindt.
 */
function pressFromShadowInput(key: string): boolean {
  const host = document.createElement('nldd-text-field');
  document.body.append(host);
  const input = document.createElement('input');
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
  Object.defineProperty(event, 'target', { value: host });
  Object.defineProperty(event, 'composedPath', { value: () => [input, host, document.body, document.documentElement, document, window] });
  window.dispatchEvent(event);
  host.remove();
  return event.defaultPrevented;
}

describe('usePresentation in het Engels', () => {
  let p: Presentation;
  let router: FakeRouter;

  beforeEach(() => {
    p = usePresentation();
    router = fakeRouter();
    p.init({ router, slides: SLIDES });
    p.setMode('zaal');
    adoptLocale('en');
  });

  afterEach(() => {
    p.stop();
    adoptLocale('nl');
  });

  it('opent het Engelse tabblad voor een dia met een Nederlands pad', async () => {
    // `route: /regelwerken` staat zo in demo-config.yaml, want dat bestand gaat over
    // dia's en hoort de routetabel van elke taal niet te kennen. Wie het dek in
    // het Engels draait hoort wel op /en/ruleworks te landen.
    await p.start(1);
    expect(router.currentRoute.value.path).toBe('/en/ruleworks');
  });

  it('neemt de wet uit het dia-pad mee naar het Engelse tabblad', async () => {
    p.init({ slides: [SLIDES[0], { kind: 'demo', title: 'De wet', route: '/regelwerken/zorgtoeslagwet' }] });
    await p.start(1);
    expect(router.currentRoute.value.path).toBe('/en/ruleworks/zorgtoeslagwet');
  });

  it('houdt de wet vast voor een dia die nog het oude pad draagt', async () => {
    // Het tabblad stond op `/wetten`. Een dia die dat pad nog heeft moet op
    // dezelfde wet uitkomen: tegen het nieuwe pad afgemeten valt de rest weg,
    // en opent het tabblad de wet die er toevallig nog open stond.
    p.init({ slides: [SLIDES[0], { kind: 'demo', title: 'De wet', route: '/wetten/zorgtoeslagwet' }] });
    await p.start(1);
    expect(router.currentRoute.value.path).toBe('/en/ruleworks/zorgtoeslagwet');
  });

  it('houdt de toetsen vast op het Engelse tabblad dat de dia opende', async () => {
    // Het dek vergelijkt op de pagina en niet op de routenaam; zou het dat wel
    // doen, dan was het doof op precies de dia die dit tabblad zojuist opende.
    await p.start(1);
    // Zaalmodus, dia met een route: het dek staat niet in beeld, maar het
    // scherm is nog van deze dia.
    expect(p.visible.value).toBe(false);
    expect(press(' ')).toBe(true);
    expect(p.index.value).toBe(2);
  });
});

describe('usePresentation toetsafvang', () => {
  let p: Presentation;
  let router: FakeRouter;

  beforeEach(() => {
    p = usePresentation();
    router = fakeRouter();
    p.init({ router, slides: SLIDES });
    p.setMode('zaal');
  });

  afterEach(() => {
    p.stop();
  });

  it('bladert op een dia die het scherm dekt', async () => {
    await p.start(0);
    expect(p.visible.value).toBe(true);
    expect(press(' ')).toBe(true);
    expect(p.index.value).toBe(1);
  });

  it('blijft bladeren op de demo die de dia zelf geopend heeft', async () => {
    await p.start(1);
    // Zaalmodus, dia met een route: het dek staat niet in beeld, maar het
    // scherm is nog van deze dia.
    expect(p.visible.value).toBe(false);
    expect(router.currentRoute.value.path).toBe('/regelwerken');
    expect(press('ArrowRight')).toBe(true);
    expect(p.index.value).toBe(2);
  });

  it('landt op de wet uit het dia-pad, ook als er een andere open stond', async () => {
    router.goTo('/regelwerken/zvw');
    p.init({ slides: [SLIDES[0], { kind: 'demo', title: 'De wet', route: '/regelwerken/zorgtoeslagwet' }] });
    await p.start(1);
    expect(router.currentRoute.value.path).toBe('/regelwerken/zorgtoeslagwet');
  });

  it('laat de toetsen los zodra de presentator zelf een ander tabblad opent', async () => {
    await p.start(1);
    router.goTo('/simulatie');

    expect(p.isOnStage()).toBe(false);
    expect(press(' ')).toBe(false);
    expect(press('ArrowRight')).toBe(false);
    expect(press('ArrowLeft')).toBe(false);
    expect(p.index.value).toBe(1);
  });

  it('pakt de toetsen weer op als de presentator terugloopt naar de dia', async () => {
    await p.start(1);
    router.goTo('/simulatie');
    expect(press(' ')).toBe(false);

    router.goTo('/regelwerken');
    expect(press(' ')).toBe(true);
    expect(p.index.value).toBe(2);
  });

  it('blijft bladeren als het tabblad zichzelf verdiept', async () => {
    await p.start(1);
    // WettenView opent bij binnenkomst meteen de standaardwet van het profiel
    // en doet `router.replace('/regelwerken/<lawId>')` (een watch met
    // `immediate: true`), zonder dat de presentator iets aanraakt. Hetzelfde
    // geldt voor ScenariosView. Vergelijken op pad zou het dek dus doof maken
    // op de dia die zojuist zelf dit tabblad opende.
    router.goTo('/regelwerken/zorgtoeslagwet');

    expect(p.isOnStage()).toBe(true);
    expect(press('ArrowRight')).toBe(true);
    expect(p.index.value).toBe(2);
  });

  it('vangt niets meer af na stop()', async () => {
    await p.start(0);
    p.stop();
    expect(press(' ')).toBe(false);
    expect(p.index.value).toBe(0);
  });

  it('Escape stopt de presentatie ook als je zelf weggelopen bent', async () => {
    await p.start(1);
    router.goTo('/simulatie');
    // De bladertoetsen zijn hier terecht los, maar Escape moet blijven werken:
    // het is de enige toets die een presentatie beeindigt zonder haar uit te
    // lopen, en zonder hem leeft ze onzichtbaar door.
    expect(p.isOnStage()).toBe(false);
    expect(press(' ')).toBe(false);

    press('Escape');
    expect(p.active.value).toBe(false);
    expect(document.documentElement.classList.contains('rr-presenting')).toBe(false);
  });

  it('Escape in een open paneel sluit dat paneel en laat het dek staan', async () => {
    await p.start(1);
    const dialog = document.createElement('dialog');
    dialog.setAttribute('open', '');
    document.body.appendChild(dialog);
    dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, composed: true }));
    expect(p.active.value).toBe(true);
    dialog.remove();
  });

  it('Escape stopt het dek niet tijdens een opname', async () => {
    await p.start(1);
    escapeStops.value = false;
    press('Escape');
    expect(p.active.value).toBe(true);
    escapeStops.value = true;
    press('Escape');
    expect(p.active.value).toBe(false);
  });

  it('Escape werkt ook gewoon met het dek in beeld', async () => {
    await p.start(0);
    press('Escape');
    expect(p.active.value).toBe(false);
  });

  it('koppelt de luisteraar los na Escape, zodat de pagina de toetsen terugkrijgt', async () => {
    await p.start(1);
    press('Escape');
    expect(p.active.value).toBe(false);

    // Na Escape is de luisteraar weg: een volgende spatie is weer van de
    // pagina en bladert niets. Dit is wat de gebruiker merkt, en het pint
    // meteen dat `stop()` de luisteraar echt loskoppelt.
    document.documentElement.classList.add('rr-sentinel');
    expect(press(' ')).toBe(false);
    expect(press('Escape')).toBe(false);
    expect(document.documentElement.classList.contains('rr-sentinel')).toBe(true);
    document.documentElement.classList.remove('rr-sentinel');
  });

  it('laat een spatie in een invoerveld van het ontwerpsysteem met rust', async () => {
    await p.start(0);
    // Het dek staat in beeld en vangt spaties af, maar niet deze: die hoort in
    // het veld. `nldd-text-field` zet zijn <input> in een open shadow root, dus
    // het event komt op de host uit en `closest('input')` vindt niets.
    expect(press(' ')).toBe(true);
    expect(p.index.value).toBe(1);

    expect(pressFromShadowInput(' ')).toBe(false);
    expect(p.index.value).toBe(1); // niet doorgebladerd
  });

  it('houdt de toetsen niet vast op een dia met een route die niet bestaat', async () => {
    // Een typefout in `route:` in demo-config.yaml valt in de catch-all
    // `/:pathMatch(.*)*`, die naar `/` redirect en geen naam draagt. De
    // presentatie belandt dus op home terwijl de dia iets anders noemt: naam
    // noch pad komt overeen, en losgelaten is de veilige kant. Liever een dia
    // die niet bladert dan een spatie die overal in de app verdwijnt.
    const redirecting = fakeRouter();
    redirecting.push = (to: string) => {
      const landed = nameFor(to) ? to : '/';
      redirecting.currentRoute.value = routeFor(landed);
      return Promise.resolve();
    };
    p.init({ router: redirecting, slides: [{ kind: 'title' }, { kind: 'demo', route: '/tikfout' }] });
    await p.start(1);

    expect(redirecting.currentRoute.value.path).toBe('/');
    expect(p.isOnStage()).toBe(false);
    expect(press(' ')).toBe(false);
  });

  it('houdt zelfstandig de toetsen vast, want dan staat het dek er altijd', async () => {
    p.setMode('zelfstandig');
    await p.start(1);
    router.goTo('/simulatie');

    expect(p.visible.value).toBe(true);
    expect(p.isOnStage()).toBe(true);
    expect(press(' ')).toBe(true);
    expect(p.index.value).toBe(2);
  });
});

describe('usePresentation en het profiel', () => {
  // Een dia zonder wet in zijn pad (`route: /regelwerken`) opent de wet van wie er
  // gekozen is: de presentator die Claudia kiest en dan start, hoort bij
  // precario te landen en niet bij de zorgtoeslag van Merijn.
  const PROFILES: Record<string, PresentationDemo['profile']['value']> = {
    merijn: { default_law: { law_path: 'zorgtoeslagwet', service: 'TOESLAGEN' }, default_feature: 'zorgtoeslagwet/scenarios/zt.feature' },
    claudia: { default_law: { law_path: 'verordening_precariobelasting/gemeenten', service: 'GEMEENTE_ROTTERDAM' }, default_feature: 'verordening_precariobelasting/gemeenten/scenarios/precario.feature' },
  };
  const IDS: Record<string, string | undefined> = { zorgtoeslagwet: 'zorgtoeslagwet', 'verordening_precariobelasting/gemeenten': 'precariobelasting_rotterdam' };

  function fakeDemo(key: string): PresentationDemo {
    const profileKey: PresentationDemo['profileKey'] = { value: key };
    return {
      profileKey,
      get profile() { return { value: PROFILES[profileKey.value ?? ''] }; },
      corpus: { value: { lawByPath: (path: string) => (IDS[path] ? { id: IDS[path] } : null) } },
      setProfile(k: string) { profileKey.value = k; },
    };
  }

  let p: Presentation;
  let router: FakeRouter;
  beforeEach(() => {
    p = usePresentation();
    router = fakeRouter();
    p.setMode('zaal');
  });
  afterEach(() => {
    p.stop();
    // `init` schrijft alleen wat je meegeeft; een demo zonder profiel laat de
    // volgende test dus geen standaardwet erven.
    p.init({ demo: { profileKey: { value: null }, profile: { value: null }, corpus: { value: null }, setProfile() {} } });
  });

  it('opent de standaardwet van het gekozen profiel', async () => {
    p.init({ router, demo: fakeDemo('claudia'), slides: SLIDES });
    await p.start(1);
    expect(router.currentRoute.value.path).toBe('/regelwerken/precariobelasting_rotterdam');
  });

  it('opent na een profielwissel op de dia de wet van het nieuwe profiel', async () => {
    p.init({ router, demo: fakeDemo('claudia'), slides: [SLIDES[0], { kind: 'demo', route: '/regelwerken', profile: 'merijn' }] });
    await p.start(1);
    expect(router.currentRoute.value.path).toBe('/regelwerken/zorgtoeslagwet');
  });

  it('laat een wet in het dia-pad voorgaan', async () => {
    p.init({ router, demo: fakeDemo('claudia'), slides: [SLIDES[0], { kind: 'demo', route: '/regelwerken/zvw' }] });
    await p.start(1);
    expect(router.currentRoute.value.path).toBe('/regelwerken/zvw');
  });
});

describe('een dek per persona', () => {
  // Dia's met `decks` horen bij het verhaal van die persona; de rest delen ze.
  // Het dek ligt vast bij het starten: de dia die in Merijns dek naar Claudia
  // wisselt, mag zijn dek niet halverwege omgooien.
  const DECK: SlideConfig[] = [
    { kind: 'title', title: 'Opening' },
    { kind: 'demo', title: 'Merijn', route: '/simulatie', profile: 'merijn', decks: ['merijn'] },
    { kind: 'demo', title: 'Claudia', route: '/zaaksysteem', profile: 'claudia' },
    { kind: 'demo', title: 'Terras', route: '/simulatie', decks: ['claudia'] },
    { kind: 'closing', title: 'Slot' },
  ];
  const demoAs = (key: string | null): PresentationDemo => ({
    profileKey: { value: key },
    profile: { value: null },
    corpus: { value: null },
    setProfile(k: string) { this.profileKey.value = k; },
  });

  let p: Presentation;
  beforeEach(() => {
    p = usePresentation();
    p.setMode('zaal');
  });
  afterEach(() => {
    p.stop();
    p.init({ demo: demoAs(null) });
  });

  it('kiest de gedeelde dia\'s plus die van de persona', () => {
    expect(deckFor(DECK, 'claudia').map((s) => s.title)).toEqual(['Opening', 'Claudia', 'Terras', 'Slot']);
    expect(deckFor(DECK, 'merijn').map((s) => s.title)).toEqual(['Opening', 'Merijn', 'Claudia', 'Slot']);
  });

  it('start met Claudia zonder naar Merijn te wisselen', async () => {
    const demo = demoAs('claudia');
    p.init({ router: fakeRouter(), demo, slides: DECK });
    await p.start(0);
    for (let i = 0; i < 3; i += 1) await p.next();
    expect(p.current.value?.title).toBe('Slot');
    expect(demo.profileKey.value).toBe('claudia');
  });

  it('houdt Merijns dek vast als een dia naar Claudia wisselt', async () => {
    const demo = demoAs('merijn');
    p.init({ router: fakeRouter(), demo, slides: DECK });
    await p.start(0);
    await p.next();
    await p.next();
    expect(demo.profileKey.value).toBe('claudia');
    await p.next();
    expect(p.current.value?.title).toBe('Slot');
  });
});

describe('een lopend dek en nieuwe dia\'s', () => {
  it('houdt het dek van de start vast, maar neemt vertaalde dia\'s over', async () => {
    const p = usePresentation();
    p.setMode('zaal');
    const demo: PresentationDemo = { profileKey: { value: 'claudia' }, profile: { value: null }, corpus: { value: null }, setProfile(k) { this.profileKey.value = k; } };
    const deck = (title: string): SlideConfig[] => [{ kind: 'title', title }, { kind: 'demo', title: 'M', route: '/simulatie', decks: ['merijn'] }, { kind: 'closing', title: 'Slot' }];
    p.init({ router: fakeRouter(), demo, slides: deck('Opening') });
    await p.start(0);
    demo.profileKey.value = 'merijn';
    p.init({ slides: deck('Opening (en)') });
    expect(p.slides.value.map((s) => s.title)).toEqual(['Opening (en)', 'Slot']);
    p.stop();
    p.init({ demo: { profileKey: { value: null }, profile: { value: null }, corpus: { value: null }, setProfile() {} } });
  });
});

describe('terugbladeren in een dek', () => {
  it('geeft dia\'s vóór de eerste persona-dia de persona van het dek terug', async () => {
    const p = usePresentation();
    p.setMode('zaal');
    const demo: PresentationDemo = { profileKey: { value: 'merijn' }, profile: { value: null }, corpus: { value: null }, setProfile(k) { this.profileKey.value = k; } };
    const slides: SlideConfig[] = [{ kind: 'title', title: 'Opening' }, { kind: 'demo', title: 'Wet', route: '/regelwerken' }, { kind: 'demo', title: 'Claudia', route: '/simulatie', profile: 'claudia' }];
    p.init({ router: fakeRouter(), demo, slides });
    await p.start(0);
    await p.goTo(2);
    expect(demo.profileKey.value).toBe('claudia');
    await p.goTo(1);
    expect(demo.profileKey.value).toBe('merijn');
    p.stop();
    p.init({ demo: { profileKey: { value: null }, profile: { value: null }, corpus: { value: null }, setProfile() {} } });
  });
});
