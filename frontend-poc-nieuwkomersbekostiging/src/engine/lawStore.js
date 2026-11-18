/**
 * lawStore - reactieve laag boven de in de engine geladen wetten.
 *
 * Verantwoordelijkheden:
 *  - de rauwe YAML-teksten bijhouden (per index-entry), zodat de editor en de
 *    diff er tegenaan kunnen en de worker exact dezelfde wetten kan draaien;
 *  - de definities (machine_readable.definitions) uit elk document platslaan
 *    tot een bewerkbare lijst, gegroepeerd per document → artikel;
 *  - een parameter-wijziging terugschrijven naar het YAML-document en de wet
 *    hot-reloaden in de engine (unload + load), met een versieteller zodat
 *    views opnieuw simuleren;
 *  - beleidsvarianten activeren: de varianten-YAML's vervangen de basis-YAML's
 *    in de engine; deactiveren zet de basis terug.
 *
 * Definitie-model (flat item):
 *   { key, lawPath, lawId, docName, article, name,
 *     value, baseValue, currentValue, type, unit, description }
 */
import { reactive, ref, computed } from 'vue';
import * as yaml from 'js-yaml';
import { b } from '../basePad.js';
import { patchDefinitionValue } from '../lib/yamlPatch.js';
import { writePersisted, readPersisted } from '../composables/useBewaardeStand.js';
import {
  browserVariants,
  browserVariantYaml,
  saveVariant as saveBrowserVariant,
  updateVariant as updateBrowserVariant,
  deleteVariant as deleteBrowserVariant,
  checkDrift,
  isBrowserVariant,
} from '@regelrecht/frontend-shared/browserVariants';
import { LAW_PO, LAW_VO, LAW_UITVOERINGSBESLUIT_WVO, LAW_WERKINSTRUCTIE } from '../lib/nieuwkomerFacts.js';

// De regelingen die de simulatie daadwerkelijk raadpleegt; alleen deze hoeven
// een editor/diff te krijgen. De wetten erboven (WPO, WVO 2020) zijn stabiele
// achtergrond en worden ongewijzigd meegeladen.
const EDITABLE_LAW_IDS = new Set([LAW_PO, LAW_VO, LAW_UITVOERINGSBESLUIT_WVO, LAW_WERKINSTRUCTIE]);

// Reactieve versieteller: elke wijziging (parameter of variant) bumpt deze,
// zodat views die erop watchen opnieuw simuleren.
const version = ref(0);

// Per index-path: { entry, baseYaml, currentYaml, doc }
const docs = reactive({});
// Volgorde van de index-paden, voor een stabiele lijstweergave.
const docPaths = ref([]);

const activeVariant = ref(null); // { id, title, files } of null
// De varianten uit de repo (variants.json), zoals ze zijn ingecheckt.
const repoVariants = ref([]);
// Wat de kiezer, de kolommen en de werkversiebalk zien: de ingecheckte
// varianten met de zelf bewaarde erachteraan. Eén lijst, zodat geen enkele
// aanroeper hoeft te weten waar een variant vandaan komt; alleen de tekst
// komt bij een eigen variant uit de browser in plaats van uit een fetch.
const variants = computed(() => [...repoVariants.value, ...browserVariants()]);

let engineRef = null;

async function fetchText(url, pogingen = 6) {
  // De basis wordt hier toegepast, niet bij elke aanroeper: de paden komen uit
  // de manifesten (index.json, variants.json) en worden op meerdere plekken
  // opgehaald. Eén vergeten plek is een wet die niet laadt, en dat blijkt pas
  // in de browser.
  const volledig = b(url);
  // Vite's cache van public/ loopt na het wegschrijven van een nieuwe variant
  // een paar honderd ms achter en geeft dan index.html (SPA-fallback) terug.
  // Herken dat en probeer kort opnieuw.
  for (let i = 0; ; i++) {
    const res = await fetch(volledig, { cache: 'no-store' });
    const type = res.headers.get('content-type') ?? '';
    const isHtml = type.includes('text/html');
    if (res.ok && !isHtml) return res.text();
    if (i >= pogingen - 1) {
      throw new Error(`Ophalen mislukt (${isHtml ? 'geen bestand' : res.status}): ${volledig}`);
    }
    await new Promise((r) => setTimeout(r, 300));
  }
}

/**
 * Initialiseer de store met de al-geladen engine en de wet-index. Haalt de
 * rauwe YAML's op (opnieuw; goedkoop en houdt de bron los van de engine) en
 * parseert ze.
 */
async function initStore(engine, lawIndex) {
  engineRef = engine;
  const paths = [];
  for (const entry of lawIndex) {
    const text = await fetchText(entry.path);
    docs[entry.path] = {
      entry,
      baseYaml: text,
      currentYaml: text,
      doc: yaml.load(text),
    };
    paths.push(entry.path);
  }
  docPaths.value = paths;

  // Varianten-index (optioneel; ontbreekt niet in deze build maar defensief).
  try {
    repoVariants.value = JSON.parse(await fetchText('laws/variants.json'));
  } catch {
    repoVariants.value = [];
  }

  // Werkversie van de vorige keer terugzetten, maar alleen als die variant er
  // nog is: een branch kan intussen verwijderd of hernoemd zijn.
  const bewaard = readPersisted('werkversie', null);
  if (bewaard && variants.value.some((v) => v.id === bewaard)) {
    await setWerkversie(bewaard);
  }
}

/** Alle documenten die een editor/diff verdienen, in vaste volgorde: eerst de
 *  po-regeling (bedragen, duur, drempel), dan vo, het besluit en de werkinstructie. */
const EDITABLE_ORDER = [LAW_PO, LAW_VO, LAW_UITVOERINGSBESLUIT_WVO, LAW_WERKINSTRUCTIE];
const editableDocs = computed(() =>
  docPaths.value
    .map((p) => docs[p])
    .filter((d) => EDITABLE_LAW_IDS.has(d.entry.id))
    .sort((a, b) => EDITABLE_ORDER.indexOf(a.entry.id) - EDITABLE_ORDER.indexOf(b.entry.id)),
);

/**
 * Vlakke lijst van alle definities uit de bewerkbare documenten. Elke definitie
 * krijgt een stabiele key (path::article::name).
 */
const definitions = computed(() => {
  // afhankelijk van version zodat currentValue meebeweegt met edits
  version.value;
  const list = [];
  for (const d of editableDocs.value) {
    const articles = d.doc?.articles ?? [];
    for (const article of articles) {
      const defs = article?.machine_readable?.definitions;
      if (!defs) continue;
      for (const [name, def] of Object.entries(defs)) {
        if (def === null || typeof def !== 'object') continue;
        list.push({
          key: `${d.entry.path}::${article.number}::${name}`,
          lawPath: d.entry.path,
          lawId: d.entry.id,
          docName: d.entry.name,
          article: article.number,
          name,
          value: def.value,
          type: def.type,
          unit: def.type_spec?.unit ?? null,
          description: def.description ?? null,
        });
      }
    }
  }
  return list;
});

/** Definities gegroepeerd per document → artikel, voor de parametereditor. */
const definitionsByDoc = computed(() => {
  const groups = new Map();
  for (const def of definitions.value) {
    if (!groups.has(def.lawPath)) {
      groups.set(def.lawPath, { docName: def.docName, lawPath: def.lawPath, articles: new Map() });
    }
    const g = groups.get(def.lawPath);
    if (!g.articles.has(def.article)) g.articles.set(def.article, []);
    g.articles.get(def.article).push(def);
  }
  // Naar een gewoon object voor eenvoudige template-iteratie.
  return [...groups.values()].map((g) => ({
    docName: g.docName,
    lawPath: g.lawPath,
    lawId: docs[g.lawPath]?.entry?.id ?? null,
    validFrom: docs[g.lawPath]?.entry?.valid_from ?? null,
    articles: [...g.articles.entries()].map(([article, defs]) => ({ article, defs })),
  }));
});

/** Lees een definitie live uit het huidige document (numeriek/tekst). */
function readDefinition(lawId, article, name) {
  version.value;
  for (const d of editableDocs.value) {
    if (d.entry.id !== lawId) continue;
    const art = (d.doc?.articles ?? []).find((a) => String(a.number) === String(article));
    const def = art?.machine_readable?.definitions?.[name];
    if (def) return def.value;
  }
  return undefined;
}

/** Herlaad één document in de engine vanuit zijn currentYaml. */
// Variant-bestanden zonder basisdocument (een nieuwe versie van een regeling,
// bijvoorbeeld 2028-01-01.yaml) worden bij activering als extra document
// toegevoegd en bij deactivering weer verwijderd.
const extraPaths = new Set();
// Cache van opgehaalde varianten-YAML per bestandspad; leeggemaakt na reloadVariants().
const variantYamlCache = new Map();

/** Herlaad alle versies van één wet in de engine (unloadLaw haalt ze allemaal weg). */
function reloadLawId(id) {
  engineRef.unloadLaw(id);
  for (const p of docPaths.value) if (docs[p].entry.id === id) engineRef.loadLaw(docs[p].currentYaml);
}

function reloadDoc(d) {
  reloadLawId(d.entry.id);
}

function removeExtraDocs() {
  if (!extraPaths.size) return;
  const ids = new Set();
  for (const p of extraPaths) { ids.add(docs[p].entry.id); delete docs[p]; }
  docPaths.value = docPaths.value.filter((p) => !extraPaths.has(p));
  extraPaths.clear();
  for (const id of ids) reloadLawId(id);
}

/**
 * Wijzig een definitie-waarde: patch het geparste document, serialiseer terug
 * naar YAML, herlaad de wet in de engine en bump de versie.
 */
function applyDefinitionChange(lawPath, article, name, value) {
  const d = docs[lawPath];
  if (!d) return;
  const art = (d.doc.articles ?? []).find((a) => String(a.number) === String(article));
  if (!art?.machine_readable?.definitions?.[name]) return;
  // Tekstpatch: één regel in de YAML, zodat de diff de wetswijziging toont en
  // commentaar en opmaak behouden blijven.
  d.currentYaml = patchDefinitionValue(d.currentYaml, article, name, value);
  d.doc = yaml.load(d.currentYaml);
  reloadDoc(d);
  version.value++;
}

/**
 * Vervang de rauwe YAML van een document (structuur-edit uit de YAML-editor).
 * Gooit door op parse/load-fouten zodat de caller ze in een banner kan tonen;
 * de engine houdt bij een fout de laatste goede versie omdat we pas na een
 * geslaagde parse unloaden.
 */
function applyRawYaml(lawPath, newYaml) {
  const d = docs[lawPath];
  if (!d) throw new Error('Onbekend document');
  const parsed = yaml.load(newYaml); // gooit bij ongeldige YAML
  // Probeer eerst te laden in een wegwerp-staat: unload+load, en bij een fout
  // de oude versie terugzetten.
  const previousYaml = d.currentYaml;
  engineRef.unloadLaw(d.entry.id);
  try {
    engineRef.loadLaw(newYaml);
  } catch (e) {
    // herstel de vorige goede versie
    engineRef.loadLaw(previousYaml);
    throw e;
  }
  d.currentYaml = newYaml;
  d.doc = parsed;
  version.value++;
}

/**
 * Werkversie-model. Er is altijd precies één werkversie: huidig recht (basis)
 * of één variant. Alles wat bewerkt (parameterpaneel, YAML-editor, assistent,
 * budgetneutraal-oplosser) werkt op de werkversie. De andere kolommen zijn
 * vast: huidig recht is de basis uit main, een variant is basis plus branch.
 *
 * Per document is `originYaml` het uitgangspunt van de werkversie: de basis,
 * of bij een variant de branchtekst. Bewerkingen = currentYaml !== originYaml.
 */
function originOf(d) {
  return d.variantYaml ?? d.baseYaml;
}

/** Zet de bewerkingen van de werkversie terug; de werkversie zelf blijft. */
function resetChanges() {
  for (const path of docPaths.value) {
    const d = docs[path];
    const origin = originOf(d);
    if (d.currentYaml !== origin) {
      d.currentYaml = origin;
      d.doc = yaml.load(origin);
      reloadDoc(d);
    }
  }
  version.value++;
}

/** Paden van documenten met bewerkingen ten opzichte van de werkversie. */
const changedPaths = computed(() => {
  version.value;
  return docPaths.value.filter((p) => docs[p].currentYaml !== originOf(docs[p]));
});
const hasChanges = computed(() => changedPaths.value.length > 0);
const changeCount = computed(() => changedPaths.value.length);

/** De werkversie: null = huidig recht, anders het variant-id. */
const werkversie = computed(() => activeVariant.value?.id ?? null);

/** "Variant nk-1: ambtshalve … (…)" → "nk-1: ambtshalve …". */
export function kortTitel(variant) {
  if (!variant) return 'Huidig recht';
  const m = String(variant.title ?? '').match(/^variant\s+([\w-]+)\s*:\s*(.*)$/i);
  if (!m) return String(variant.title ?? variant.id).split('(')[0].trim();
  return `${m[1]}: ${m[2].split('(')[0].trim()}`;
}

/**
 * Nog korter, voor chips en tabelkoppen: alleen het variantnummer en de kern
 * van de titel. De volle titel is soms zeventig tekens en past dan op een
 * smal scherm niet in een knop, die niet afbreekt.
 */
export function chipTitel(variant, max = 28) {
  const kort = kortTitel(variant);
  if (kort.length <= max) return kort;
  const dp = kort.indexOf(': ');
  if (dp === -1) return `${kort.slice(0, max - 1).trimEnd()}…`;
  const nummer = kort.slice(0, dp);
  const rest = kort.slice(dp + 2);
  const ruimte = max - nummer.length - 3;
  // Knip op een woordgrens, zodat er geen half woord overblijft.
  const knip = rest.lastIndexOf(' ', ruimte);
  return `${nummer}: ${rest.slice(0, knip > 8 ? knip : ruimte).trimEnd()}…`;
}

const werkversieLabel = computed(() => kortTitel(activeVariant.value ? variants.value.find((v) => v.id === activeVariant.value.id) ?? activeVariant.value : null));

/** Haal variants.json opnieuw op (na "Bewaar als variant"). */
async function reloadVariants() {
  try {
    repoVariants.value = JSON.parse(await fetchText(`/laws/variants.json?t=${Date.now()}`));
  } catch {
    repoVariants.value = [];
  }
  variantYamlCache.clear();
}

/**
 * Kies de werkversie. null = huidig recht (basis uit main). Bewerkingen van
 * de vorige werkversie gaan verloren; de aanroeper vraagt daar zelf om.
 */
async function setWerkversie(variantId = null) {
  // Onthouden waarop je bewerkt, zodat een ververs je niet terugzet op
  // huidig recht. De bewerkingen zelf bewaren we niet; die horen in een
  // variant-branch.
  writePersisted('werkversie', variantId ?? null);
  if (!variantId) {
    removeExtraDocs();
    for (const path of docPaths.value) {
      const d = docs[path];
      delete d.variantYaml;
      if (d.currentYaml !== d.baseYaml) {
        d.currentYaml = d.baseYaml;
        d.doc = yaml.load(d.baseYaml);
        reloadDoc(d);
      }
    }
    activeVariant.value = null;
    version.value++;
    return;
  }
  await activateVariant(variantId);
}

/**
 * De bestanden van de werkversie die afwijken van main, als
 * [{ path (repo-relatief), yaml }]: de bewerkte basisdocumenten en, bij een
 * variant, alle bestanden van de branch (nieuwe versies inbegrepen). Dit is
 * wat "Bewaar als variant" naar de server stuurt.
 */
function editedFilesForSave() {
  const out = [];
  for (const path of docPaths.value) {
    const d = docs[path];
    const nieuw = extraPaths.has(path);
    if (!nieuw && d.currentYaml === d.baseYaml) continue;
    const rel = String(path).replace(/^\/laws\//, '');
    out.push({ path: `corpus/${rel}`, yaml: d.currentYaml });
  }
  return out;
}

/**
 * Dezelfde bestanden, maar met het indexpad dat de app gebruikt en met de
 * basistekst erbij. Dat laatste is het uitgangspunt waarop bewerkt is: daarmee
 * kan later gemeld worden dat het corpus onder de variant vandaan is gewijzigd.
 *
 * Afgezet tegen `baseYaml` en niet tegen de werkversie, zodat een variant die
 * op een andere variant voortbouwt zichzelf compleet meedraagt en dus zonder
 * die ander te activeren is.
 */
function editedFilesForBrowserVariant() {
  const out = [];
  for (const path of docPaths.value) {
    const d = docs[path];
    const nieuw = extraPaths.has(path);
    if (!nieuw && d.currentYaml === d.baseYaml) continue;
    out.push({ path, yaml: d.currentYaml, original: nieuw ? null : d.baseYaml });
  }
  return out;
}

/**
 * Bewaar de werkversie als eigen variant in de browser, en zet hem meteen als
 * werkversie. Geeft de nieuwe variant terug.
 *
 * Lukt het activeren niet (de engine keurt de bewerkte wet af), dan gaat de
 * variant er weer uit, valt de werkversie terug op huidig recht en komt de
 * fout naar boven. Anders bleef er een variant staan die de app niet kan
 * laden, terwijl het scherm "bewaren mislukt" meldt: de gebruiker ziet hem dan
 * wel in de lijst en loopt er later opnieuw tegenaan.
 */
async function bewaarAlsBrowserVariant(titel) {
  const bestanden = editedFilesForBrowserVariant();
  const variant = saveBrowserVariant({
    title: titel,
    files: bestanden,
    base: werkversie.value,
    // Ook de ingecheckte id's, zodat een eigen variant er nooit een overschaduwt.
    takenIds: variants.value.map((v) => v.id),
  });
  try {
    await setWerkversie(variant.id);
  } catch (e) {
    deleteBrowserVariant(variant.id);
    // De activering is halverwege gestrand, dus de engine draagt nu een
    // mengsel. Terug naar de basis, die altijd laadt.
    await setWerkversie(null).catch(() => {});
    throw e;
  }
  return variant;
}

/**
 * Werk de eigen variant bij waar je nu in werkt: dezelfde variant, met de
 * bewerkingen die er sindsdien bij zijn gekomen.
 *
 * Zonder dit kon een variant alleen groeien. De gebruikelijke gang is een
 * variant maken, doorrekenen, een parameter bijstellen en opnieuw bewaren; dat
 * leverde elke keer een bijna-gelijke variant naast de vorige op, met alleen
 * een nummer als verschil. De browseropslag heeft bovendien een grens.
 *
 * Na afloop laadt de werkversie opnieuw, zodat de app met de bewaarde tekst
 * rekent en niet met de bewerkingen die er los overheen lagen.
 */
async function werkWerkversieBij() {
  const id = werkversie.value;
  if (!isBrowserVariant(id)) {
    throw new Error('Alleen een eigen variant is bij te werken.');
  }
  const { variant, restore } = updateBrowserVariant(id, { files: editedFilesForBrowserVariant() });
  try {
    // Opnieuw activeren wist de losse bewerkingen: die zitten nu in de variant
    // zelf. Zonder dit zou "Terugzetten" ze daarna alsnog weggooien terwijl ze
    // bewaard zijn.
    await setWerkversie(id);
  } catch (e) {
    // De engine keurt de bewerkte wet af. Anders dan bij een nieuwe variant
    // valt hier niets weg te gooien: deze variant bestond al en de vorige
    // inhoud is net overschreven. Die gaat terug, en de werkversie laadt hem
    // opnieuw, zodat er geen variant achterblijft die nooit meer opent.
    restore();
    await setWerkversie(id).catch(async () => { await setWerkversie(null).catch(() => {}); });
    throw e;
  }
  return variant;
}

/**
 * Gooi een eigen variant weg. Was hij de werkversie, dan valt de app terug op
 * huidig recht: doorwerken op een variant die er niet meer is, levert een
 * kolom op die niets meer voorstelt.
 */
async function verwijderEigenVariant(id) {
  const wasWerkversie = werkversie.value === id;
  const weg = deleteBrowserVariant(id);
  if (weg && wasWerkversie) await setWerkversie(null);
  else if (weg) version.value++;
  return weg;
}

/**
 * Is het corpus gewijzigd sinds deze eigen variant is bewaard? Geeft
 * `{ drifted, paths, unknown }`; voor een ingecheckte variant altijd
 * "niets aan de hand", want die beweegt met de repo mee.
 */
function variantDrift(id) {
  if (!isBrowserVariant(id)) return { drifted: false, paths: [], unknown: false };
  return checkDrift(id, (pad) => docs[pad]?.baseYaml);
}

/**
 * Activeer een beleidsvariant: laad per bestand de varianten-YAML in de engine
 * (vervangt de basis) en houd de rauwe tekst bij voor de diff. Wist eerst
 * eventuele andere variant/edits terug naar basis.
 */
async function activateVariant(variantId) {
  const variant = variants.value.find((v) => v.id === variantId);
  if (!variant) return;
  // Begin schoon vanaf de basis (zonder de versieteller dubbel te bumpen).
  removeExtraDocs();
  for (const path of docPaths.value) {
    const d = docs[path];
    delete d.variantYaml;
    if (d.currentYaml !== d.baseYaml) {
      d.currentYaml = d.baseYaml;
      d.doc = yaml.load(d.baseYaml);
      reloadDoc(d);
    }
  }
  for (const file of variant.files) {
    const d = docs[file.base];
    if (!d) {
      // Nieuw versiebestand: als extra document registreren en laden.
      const text = await fetchVariantYaml(file.path, variantId);
      const parsed = yaml.load(text);
      docs[file.base] = {
        entry: { id: parsed.$id, name: parsed.name, regulatory_layer: parsed.regulatory_layer, valid_from: parsed.valid_from, path: file.base },
        baseYaml: text,
        currentYaml: text,
        doc: parsed,
      };
      docPaths.value = [...docPaths.value, file.base];
      extraPaths.add(file.base);
      reloadLawId(parsed.$id);
      continue;
    }
    const variantYaml = await fetchVariantYaml(file.path, variantId);
    d.variantYaml = variantYaml;
    d.currentYaml = variantYaml;
    d.doc = yaml.load(variantYaml);
    reloadDoc(d);
  }
  activeVariant.value = { id: variant.id, title: variant.title, files: variant.files };
  version.value++;
}

/** Terug naar huidig recht als werkversie. */
function deactivateVariant() {
  return setWerkversie(null);
}

/** De YAML-teksten van alle documenten, in index-volgorde, voor de worker. */
function currentLawYamls() {
  return docPaths.value.map((p) => docs[p].currentYaml);
}

/**
 * Neem de overlays van de beleidsassistent over: per document_key de nieuwe
 * YAML in de engine laden en de store bijwerken, zodat editor/diff/persona's
 * het assistent-resultaat weerspiegelen. document_key wordt gematcht op het
 * law-$id (entry.id) of, als fallback, op het pad-einde.
 */
/**
 * Het documentpad bij een sleutel van de assistent. Die gebruikt
 * "wet_id@valid_from"; match daarom op id en versie, met het kale id en het
 * pad als terugval.
 */
function docPathVoorKey(key) {
  const [keyId, keyVersie] = decodeURIComponent(String(key)).split('@');
  return docPaths.value.find((p) => {
    const d = docs[p];
    if (keyVersie) return d.entry.id === keyId && String(d.entry.valid_from) === keyVersie;
    return d.entry.id === key || p.endsWith(key) || p.includes(key);
  }) ?? null;
}

function applyOverlays(overlays) {
  for (const [key, yamlText] of Object.entries(overlays ?? {})) {
    const path = docPathVoorKey(key);
    if (!path) {
      console.warn('Overlay van de assistent past op geen document:', key);
      continue;
    }
    const d = docs[path];
    try {
      const parsed = yaml.load(yamlText);
      engineRef.unloadLaw(d.entry.id);
      engineRef.loadLaw(yamlText);
      d.currentYaml = yamlText;
      d.doc = parsed;
    } catch {
      // ongeldige overlay overslaan; de laatste goede versie blijft
    }
  }
  version.value++;
}

/** Rauwe basis- en huidige YAML van één document (voor de diff-sheet). */
function docYaml(lawPath) {
  const d = docs[lawPath];
  return d ? { baseYaml: d.baseYaml, currentYaml: d.currentYaml, doc: d.doc, entry: d.entry } : null;
}


/**
 * De YAML van één variantbestand. Dit is de enige plek waar de tekst van een
 * variant vandaan komt, en daarmee ook de enige plek die hoeft te weten dat
 * een eigen variant in de browser staat in plaats van in public/.
 */
async function fetchVariantYaml(path, variantId = null) {
  if (isBrowserVariant(variantId)) {
    const tekst = browserVariantYaml(variantId, path);
    if (typeof tekst !== 'string') {
      throw new Error(`Variant ${variantId} heeft geen bewaarde tekst voor ${path}`);
    }
    return tekst;
  }
  if (!variantYamlCache.has(path)) variantYamlCache.set(path, await fetchText(path));
  return variantYamlCache.get(path);
}

/**
 * De documenten van een kolom als [{ path, yaml, entry }]. De werkversie
 * (huidig recht of één variant) levert de bewerkte documenten; elke andere
 * kolom is vast: huidig recht = de basis uit main, een variant = basis plus
 * de bestanden van de branch (nieuwe versies inbegrepen).
 */
async function lawDocsFor(variantId = null) {
  version.value;
  const variant = variantId ? variants.value.find((v) => v.id === variantId) : null;
  const isWerkversie = (variantId ?? null) === werkversie.value;
  const out = [];
  for (const path of docPaths.value) {
    if (isWerkversie) {
      out.push({ path, yaml: docs[path].currentYaml, entry: docs[path].entry });
      continue;
    }
    // Extra documenten horen bij de werkversie, niet bij deze kolom.
    if (extraPaths.has(path)) continue;
    const file = variant?.files?.find((f) => f.base === path);
    out.push({ path, yaml: file ? await fetchVariantYaml(file.path, variantId) : docs[path].baseYaml, entry: docs[path].entry });
  }
  if (!isWerkversie) {
    // Nieuwe versiebestanden van deze variant (zonder basisdocument) erbij.
    for (const file of variant?.files ?? []) {
      // Een basisdocument is hierboven al gedaan; een extra document (nieuw
      // versiebestand van de werkversie) hoort bij de werkversie, dus hier
      // komt de branchversie van dit bestand alsnog mee.
      if (docPaths.value.includes(file.base) && !extraPaths.has(file.base)) continue;
      const text = await fetchVariantYaml(file.path, variantId);
      const parsed = yaml.load(text);
      out.push({ path: file.base, yaml: text, entry: { id: parsed.$id, name: parsed.name, regulatory_layer: parsed.regulatory_layer, valid_from: parsed.valid_from, path: file.base } });
    }
  }
  return out;
}

async function lawYamlsFor(variantId = null) {
  return (await lawDocsFor(variantId)).map((d) => d.yaml);
}

export function useLawStore() {
  return {
    version,
    variants,
    activeVariant,
    definitions,
    definitionsByDoc,
    editableDocs,
    hasChanges,
    changeCount,
    changedPaths,
    werkversie,
    werkversieLabel,
    setWerkversie,
    reloadVariants,
    editedFilesForSave,
    bewaarAlsBrowserVariant,
    werkWerkversieBij,
    verwijderEigenVariant,
    variantDrift,
    isBrowserVariant,
    initStore,
    readDefinition,
    applyDefinitionChange,
    applyRawYaml,
    resetChanges,
    activateVariant,
    deactivateVariant,
    currentLawYamls,
    lawYamlsFor,
    lawDocsFor,
    docPaths,
    docYaml,
    docPathVoorKey,
    applyOverlays,
  };
}
