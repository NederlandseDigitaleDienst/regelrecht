// Centralised URL builders for the editor's corpus endpoints. The two
// shapes - global (`/api/corpus/...`, read-only) and traject-scoped
// (`/api/trajects/{ref}/corpus/...`, read + write) - are picked by
// whether a traject ref is present at call time. Putting the choice in
// one place keeps composables aligned and prevents the pattern from
// drifting per file.
//
// The `trajectRef` is the URL-form identifier `{slug}-{8hex}` returned
// by the backend on `t.ref`. The backend resolves it back to a UUID via
// `resolve_traject_ref` - frontend code never needs to know the UUID
// shape, only the ref string.

// Callers pass a ref, or null/undefined/'' when no traject is active.
type TrajectRef = string | null | undefined;

export function corpusBase(trajectRef: TrajectRef): string {
  return trajectRef
    ? `/api/trajects/${encodeURIComponent(trajectRef)}/corpus`
    : `/api/corpus`;
}

export function lawUrl(trajectRef: TrajectRef, lawId: string): string {
  return `${corpusBase(trajectRef)}/laws/${encodeURIComponent(lawId)}`;
}

// All versions of a law (newest-first), as a JSON array of YAML strings.
// Unlike `lawUrl` (one "best for today" body), the scenario dependency loader
// feeds every version to the engine so its date-aware version selection can
// pick the one in force on the scenario's calculation date - otherwise a
// referenced law that has a future-dated version would load only that future
// version and fail "not yet in force" for a past-dated scenario.
export function lawVersionsUrl(trajectRef: TrajectRef, lawId: string): string {
  return `${lawUrl(trajectRef, lawId)}/versions`;
}

// The outputs a law offers to other laws (name, type, parameters), for the
// `source.output` picker. A law that exists only in the traject has outputs
// only on the traject route.
export function lawOutputsUrl(trajectRef: TrajectRef, lawId: string): string {
  return `${lawUrl(trajectRef, lawId)}/outputs`;
}

export function lawsListUrl(trajectRef: TrajectRef, query = ''): string {
  const q = query ? `?${query}` : '';
  return `${corpusBase(trajectRef)}/laws${q}`;
}

// Law ids edited in a traject (branch-vs-base diff). Only exists under the
// traject prefix - there is no global "changed laws" notion - so this is
// traject-only, like the documents builders. Callers already short-circuit
// the no-traject case (see `fetchChangedLawIds`); the guard here documents
// that contract and fails loudly if a future caller forgets it.
export function changedLawsUrl(trajectRef: TrajectRef): string {
  requireTraject(trajectRef, 'changed-laws listing');
  return `${corpusBase(trajectRef)}/changed-laws`;
}

// Favorieten horen bij de plek waar je werkt: in een traject de wetten van dat
// traject, in Corpus juris wat je bewaart terwijl je bladert. Beide sets staan
// los van elkaar, dus de URL kiest de set. Geen `requireTraject`: zonder traject
// is de globale route het juiste antwoord en niet een fout.
export function favoritesUrl(trajectRef: TrajectRef): string {
  return trajectRef ? `/api/trajects/${encodeURIComponent(trajectRef)}/favorites` : '/api/favorites';
}

export function favoriteUrl(trajectRef: TrajectRef, lawId: string): string {
  return `${favoritesUrl(trajectRef)}/${encodeURIComponent(lawId)}`;
}

// De bronnen van een traject: id, naam, priority, law_count en index_error
// per bron. Zit náást de corpus-prefix (`/api/trajects/{ref}/sources`), niet
// erin - vandaar de eigen opbouw in plaats van `corpusBase`. Traject-only:
// de globale tegenhanger `/api/sources` heeft geen aanroeper in de editor.
export function trajectSourcesUrl(trajectRef: TrajectRef): string {
  requireTraject(trajectRef, 'sources listing');
  return `/api/trajects/${encodeURIComponent(trajectRef)}/sources`;
}

// Law ids whose articles `implements` an open_term of `lawId` (the IoC
// reverse link). Computed server-side over the in-memory corpus, so the
// scenario dependency loader resolves implementing regulations with a
// single request instead of fetching and parsing every law in the corpus.
export function implementorsUrl(trajectRef: TrajectRef, lawId: string): string {
  return `${lawUrl(trajectRef, lawId)}/implementors`;
}

export function scenariosListUrl(trajectRef: TrajectRef, lawId: string): string {
  return `${lawUrl(trajectRef, lawId)}/scenarios`;
}

export function scenarioFileUrl(trajectRef: TrajectRef, lawId: string, filename: string): string {
  return `${scenariosListUrl(trajectRef, lawId)}/${encodeURIComponent(filename)}`;
}

export function annotationsUrl(trajectRef: TrajectRef, lawId: string): string {
  return `${lawUrl(trajectRef, lawId)}/annotations`;
}

// Documents live under the traject-scope only. The list endpoint
// returns every document in the traject's documents folder, the file
// endpoint reads/writes one specific path. Both forms require a
// trajectRef - there is no global-scope counterpart by design.
export function documentsListUrl(trajectRef: TrajectRef): string {
  requireTraject(trajectRef, 'documents listing');
  return `${corpusBase(trajectRef)}/documents`;
}

// The document path is hierarchical (e.g. "mvt/concept.md"), so each
// segment is encoded individually instead of `encodeURIComponent`-ing
// the whole thing - that would turn `/` into `%2F` and break the
// axum wildcard match.
export function documentFileUrl(trajectRef: TrajectRef, docPath: string): string {
  requireTraject(trajectRef, 'document access');
  const encoded = docPath
    .split('/')
    .map(encodeURIComponent)
    .join('/');
  return `${corpusBase(trajectRef)}/documents/${encoded}`;
}

// Multipart upload of a document; the backend stores the bytes and enqueues
// an async conversion-to-markdown job. Traject-scoped only, like the other
// document builders.
//
// `allowLlm` is de keuze van de gebruiker in de uploadbevestiging en gaat mee
// als query-parameter, niet als multipart-veld: de backend leest het
// multipart-body maar tot en met het `file`-veld, dus een extra veld zou
// stilzwijgend van de veldvolgorde afhangen. Weglaten = geen toestemming; de
// parameter alleen zetten wanneer hij `true` is houdt de URL eerlijk kort.
export function documentUploadUrl(trajectRef: TrajectRef, { allowLlm = false }: { allowLlm?: boolean } = {}): string {
  requireTraject(trajectRef, 'document upload');
  const base = `${corpusBase(trajectRef)}/documents/upload`;
  return allowLlm ? `${base}?llm=1` : base;
}

// Upload a source document that becomes a NEW law in the traject: the
// backend converts it to a base-law YAML (law_convert job), chains an
// enrichment, and delivers the result as a `law_create` review task.
export function lawUploadUrl(trajectRef: TrajectRef): string {
  requireTraject(trajectRef, 'law upload');
  return `${corpusBase(trajectRef)}/laws/upload`;
}

// Create a NEW law in the traject (the approve-step of a `law_create`
// review task). POST body = the full law YAML; the corpus path is
// derived server-side from the validated body.
export function lawCreateUrl(trajectRef: TrajectRef): string {
  requireTraject(trajectRef, 'law create');
  return `${corpusBase(trajectRef)}/laws`;
}

// Promote: kopieer een wet uit het CENTRALE corpus (alle versie-YAML's +
// scenario's) naar de traject-repo, via het gewone traject-schrijfpad.
export function lawPromoteUrl(trajectRef: TrajectRef, lawId: string): string {
  requireTraject(trajectRef, 'law promote');
  return `${lawUrl(trajectRef, lawId)}/promote`;
}

// Traject-scoped harvest via het taken-mechanisme: POST { bwb_id } start een
// async BWB-download + geketende enrich; het resultaat komt terug als
// `law_create`-review-taak in het takenpaneel.
export function trajectHarvestUrl(trajectRef: TrajectRef): string {
  requireTraject(trajectRef, 'traject harvest');
  return `${corpusBase(trajectRef)}/harvest`;
}

// Running/failed document-conversion jobs for the traject, backing the
// werkdocumenten conversion-status block.
export function documentJobsUrl(trajectRef: TrajectRef): string {
  requireTraject(trajectRef, 'document jobs listing');
  return `${corpusBase(trajectRef)}/documents/jobs`;
}

// A single conversion job — DELETE cancels it (kills a stuck upload).
export function documentJobUrl(trajectRef: TrajectRef, jobId: string): string {
  requireTraject(trajectRef, 'document job cancel');
  return `${corpusBase(trajectRef)}/documents/jobs/${encodeURIComponent(jobId)}`;
}

// Writes only exist under the traject prefix. Composables call this at
// the top of their save function so the call-stack failure is "no
// traject" instead of a malformed URL.
export function requireTraject(trajectRef: TrajectRef, op = 'this operation'): asserts trajectRef is string {
  if (!trajectRef) {
    throw new Error(`Cannot perform ${op} without an active traject`);
  }
}
