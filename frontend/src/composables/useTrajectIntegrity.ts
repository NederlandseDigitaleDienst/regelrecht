/**
 * useTrajectIntegrity - haalt het integriteitsrapport van een traject op
 * (`GET /api/trajects/:trajectRef/integrity`).
 *
 * Het rapport is een momentopname van de traject-branch: elke aanroep laat de
 * backend de repo opnieuw doorlopen (die cachet zelf op blob-sha's, dus een
 * herhaalde aanroep zonder nieuwe commits is goedkoop). De pagina laadt daarom
 * bij openen en bij een expliciete verversing - er wordt niet gepolld.
 *
 * Zelfde vorm als `useTrajectDetail`: verse state per aanroep, `loading` en
 * `error` als refs, en de state wordt vóór de await gewist zodat een tweede
 * aanroep niet even het vorige rapport laat staan.
 */
import { computed, ref } from 'vue';
import { apiFetchJson } from '../lib/apiFetch';

/** One finding (Finding in traject_integrity.rs). */
export interface IntegrityFinding {
  severity: string;
  kind: string;
  path?: string;
  law_id?: string;
  message: string;
  remedy: string;
}

/** `GET /api/trajects/:ref/integrity` (IntegrityReport in traject_integrity.rs). */
export interface IntegrityReport {
  traject_ref: string;
  source_id: string;
  checked_laws: number;
  checked_scenarios: number;
  findings: IntegrityFinding[];
}

export interface FindingGroup {
  key: string;
  title: string;
  counts: string;
  errorCount: number;
  warningCount: number;
  findings: IntegrityFinding[];
}

/**
 * Volgorde waarin bevindingen binnen een wet-groep staan: fouten (breken nu
 * iets) boven waarschuwingen (waarschijnlijk een vergissing, werkt nog wel).
 */
export const SEVERITY_ORDER: readonly string[] = ['error', 'warning'];

/** Icoon per severity, voor het icoon vóór elke bevinding. */
export const SEVERITY_ICONS: Record<string, string> = { error: 'error', warning: 'warning' };

/** Groepstitel voor bevindingen die niet bij één wet horen. */
const TRAJECT_WIDE_TITLE = 'Traject-breed';

/**
 * De wet-map waar een bevinding bij hoort, afgeleid uit het pad. Alle checks
 * wijzen met `path` een plek in de map van een wet aan: de map zelf, een
 * versiebestand erin, of (een bestand in) de `scenarios/`-map ernaast. De
 * mapnaam - niet het `$id` - is de groepssleutel: bij een mapnaam≠$id-mismatch
 * is de map het enige anker dat alle bevindingen van die wet delen.
 */
function lawDirFromPath(path: string | null | undefined): string | null {
  const segments = (path ?? '').split('/').filter(Boolean);
  // Laatste segment met een punt is een bestandsnaam (wetten heten
  // `<datum>.yaml`, scenario's `<naam>.feature`), geen map.
  // The length check guarantees the last segment exists.
  if (segments.length && segments[segments.length - 1]!.includes('.')) segments.pop();
  if (segments.length && segments[segments.length - 1] === 'scenarios') segments.pop();
  return segments[segments.length - 1] ?? null;
}

/** "2 fouten", "1 waarschuwing" - de teller in een groepskop. */
function countsLabel(findings: readonly Pick<IntegrityFinding, 'severity'>[]): string {
  const errors = findings.filter((f) => f.severity === 'error').length;
  const warnings = findings.filter((f) => f.severity === 'warning').length;
  const parts: string[] = [];
  if (errors) parts.push(`${errors} ${errors === 1 ? 'fout' : 'fouten'}`);
  if (warnings) parts.push(`${warnings} ${warnings === 1 ? 'waarschuwing' : 'waarschuwingen'}`);
  const rest = findings.length - errors - warnings;
  if (rest) parts.push(`${rest} overig`);
  return parts.join(', ');
}

/**
 * Groepeer de bevindingen van een rapport per wet (mapnaam uit het pad), de
 * zwaarst getroffen wet eerst. Zo leest een lang rapport als "deze wetten
 * hebben aandacht nodig" in plaats van één ongesorteerde foutenlijst.
 *
 * - Binnen een groep: fouten boven waarschuwingen (`SEVERITY_ORDER`); een
 *   onbekende severity (nieuwe backend, oude frontend) belandt achteraan in
 *   plaats van te verdwijnen.
 * - Groepen onderling: meeste fouten eerst, dan meeste waarschuwingen, dan
 *   alfabetisch - impact bovenaan.
 * - Bevindingen zonder pad krijgen samen de groep "Traject-breed".
 *
 */
export function groupByLaw(
  report: { findings?: readonly IntegrityFinding[] | null } | null | undefined,
): FindingGroup[] {
  const findings = report?.findings ?? [];
  const byLaw = new Map<string, IntegrityFinding[]>();
  for (const finding of findings) {
    const dir = lawDirFromPath(finding?.path);
    const key = dir ?? TRAJECT_WIDE_TITLE;
    if (!byLaw.has(key)) byLaw.set(key, []);
    // Set on the line above when missing.
    byLaw.get(key)!.push(finding);
  }
  const severityRank = (f: IntegrityFinding | null | undefined) => {
    const i = f ? SEVERITY_ORDER.indexOf(f.severity) : -1;
    return i === -1 ? SEVERITY_ORDER.length : i;
  };
  const groups = [...byLaw.entries()].map(([key, group]) => {
    const sorted = [...group].sort((a, b) => severityRank(a) - severityRank(b));
    return {
      key,
      title: key,
      counts: countsLabel(sorted),
      errorCount: sorted.filter((f) => f.severity === 'error').length,
      warningCount: sorted.filter((f) => f.severity === 'warning').length,
      findings: sorted,
    };
  });
  groups.sort(
    (a, b) =>
      b.errorCount - a.errorCount ||
      b.warningCount - a.warningCount ||
      a.title.localeCompare(b.title),
  );
  return groups;
}

/**
 * Samenvattende impactregel boven de groepen: het totaal, en over hoeveel
 * wetten het verdeeld is. Geeft een lang rapport in één zin zijn maat.
 */
export function impactSummary(
  report: { findings?: readonly IntegrityFinding[] | null } | null | undefined,
): string | null {
  const groups = groupByLaw(report);
  if (!groups.length) return null;
  const total = countsLabel(groups.flatMap((g) => g.findings));
  const laws = groups.filter((g) => g.title !== TRAJECT_WIDE_TITLE).length;
  const spread =
    laws > 1 ? `, verdeeld over ${laws} regelwerken` : laws === 1 ? ', in één regelwerk' : '';
  return `In totaal ${total}${spread}.`;
}

export function useTrajectIntegrity() {
  const report = ref<IntegrityReport | null>(null);
  const loading = ref(false);
  const error = ref<unknown>(null);

  async function load(trajectRef: string | null | undefined): Promise<void> {
    if (!trajectRef) return;
    // Wissen vóór de await: een tweede aanroep (verversen, ander traject) mag
    // het oude rapport niet laten staan terwijl het nieuwe onderweg is.
    loading.value = true;
    error.value = null;
    report.value = null;
    try {
      report.value = await apiFetchJson<IntegrityReport>(
        `/api/trajects/${encodeURIComponent(trajectRef)}/integrity`,
        {
          // De backend schrijft zijn eigen Nederlandse uitleg in de body
          // (bijv. "GitHub is nu niet bereikbaar..."); die is nuttiger dan
          // een statuscode, dus die tonen we als hij er is.
          errorMessage: (status, body) =>
            body?.trim() || `Integriteitscontrole mislukt: ${status}`,
        },
      );
    } catch (e) {
      error.value = e;
    } finally {
      loading.value = false;
    }
  }

  const groups = computed(() => groupByLaw(report.value));
  const summary = computed(() => impactSummary(report.value));
  const hasFindings = computed(() => (report.value?.findings?.length ?? 0) > 0);

  return { report, groups, summary, hasFindings, loading, error, load };
}
