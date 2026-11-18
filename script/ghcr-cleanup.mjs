#!/usr/bin/env node
// Ruimt container-versies op in GHCR, zonder de manifesten te raken waar een
// getagde image nog naar wijst of waar productie op draait.
//
// Dit is het enige opruimpad voor images. Drie soorten afval, één script, één
// stap in scheduled-cleanup.yml:
//
//   1. getagde `pr-*`-versies van gesloten pull requests;
//   2. getagde versies waarvan élke tag de `sha-`-vorm heeft, die nergens meer
//      draaien en ouder zijn dan de retentietermijn;
//   3. untagged versies waar geen getagde index meer naar wijst (de wezen).
//
// Ze staan hier bij elkaar omdat ze over dezelfde packages lopen en dezelfde
// bescherming nodig hebben. Twee opruimers die onafhankelijk over dezelfde
// registry gaan, kennen elkaars uitzonderingen niet: de een gaat op de tagvorm
// af, de ander op de referentiegraaf, en samen halen ze weg wat elk apart had
// laten staan.
//
// Het probleem dat dit script oplost: buildx zet provenance standaard aan, dus
// elke push levert een OCI *index* met de tag plus twee untagged children (het
// platform-image en een attestation-manifest). Van de untagged versies in een
// package is daardoor het merendeel geen afval maar een levend onderdeel van
// een getagde image. Een opruimer die "alles zonder tag" weggooit sloopt dus
// élke image in de registry, inclusief wat er in productie draait. De vorige
// opruimstap ging de andere kant op en sloeg untagged versies volledig over,
// waardoor de echte wezen — children waar geen tag meer naar wijst omdat
// `latest` of `pr-N` naar een nieuwe index is verschoven — voor onbepaalde
// tijd bleven staan.
//
// De aanpak is daarom: eerst de referentiegraaf opbouwen (per getagde versie
// het manifest ophalen en alle `.manifests[].digest` verzamelen), en pas
// daarna de untagged versies verwijderen die in die verzameling ontbreken.
//
// Dit is destructieve code tegen een productieregistry, dus elke twijfel valt
// de kant van "niets verwijderen" op:
//
//   * Lukt het niet om bij ZAD op te halen wát er op dit moment draait, dan
//     stopt de run zonder één DELETE. Productie draait op een `sha-`-tag en
//     niet op `latest`, dus zonder die lijst is niet vast te stellen welke
//     `sha-`-versie het draaiende image is.
//   * Kan een manifest van een getagde versie niet worden opgehaald, dan is de
//     graaf onvolledig en wordt er voor dat package géén enkele wees
//     verwijderd. Liever een ronde niets opruimen dan één keer te veel weg.
//   * Untagged versies jonger dan de respijttermijn blijven staan. Een push
//     die tijdens deze run binnenkomt kan children hebben die in onze listing
//     nog geen ouder hebben.
//   * Versies met een beschermde tag worden nooit verwijderd, ook niet als er
//     verder een stale `pr-*`-tag op zit. Beschermd zijn `latest` en elke tag
//     die ZAD op dit moment draait — productie én de nog levende previews.
//   * Vlak voor het verwijderen wordt de deletelijst nog een keer tegen de
//     referentieverzameling gehouden. Zit er een gerefereerde digest in, dan
//     stopt de run zonder iets te verwijderen.
//
// Die twee laatste hekken werken samen voor het draaiende image: de index
// waar productie op staat is getagd en dus beschermd, en omdat hij een wortel
// van de referentiegraaf is staan zijn untagged children in `referenced` en
// vallen ze nooit onder de wezen.
//
// Gebruik:
//   GH_TOKEN=… ZAD_API_KEY=… node script/ghcr-cleanup.mjs --dry-run
//   GH_TOKEN=… ZAD_API_KEY=… node script/ghcr-cleanup.mjs --packages regelrecht-docs
//
// Het token heeft `read:packages` nodig om te plannen en `delete:packages` om
// daadwerkelijk op te ruimen.

import { appendFileSync, realpathSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// De containerpackages die deze repo naar GHCR schrijft. Staat hier en niet in
// de workflow, zodat de lijst en de logica die hem gebruikt bij elkaar blijven.
export const PACKAGES = [
  'regelrecht-editor',
  'regelrecht-admin',
  'regelrecht-harvester-worker',
  'regelrecht-enrich-worker',
  'regelrecht-pipeline-api',
  'regelrecht-grafana',
  'regelrecht-lawmaking',
  'regelrecht-docs',
  'regelrecht-demo',
  'regelrecht-poc',
  'regelrecht-poc-napp',
  // Geen image meer sinds chronolexografie in de browser draait (in het
  // poc-image). Blijft hier staan zodat de oude versies worden opgeruimd.
  'regelrecht-poc-chronolexografie',
];

// Tags die een versie onaanraakbaar maken, wat er verder ook op zit. Dit is de
// bodem; de tags die ZAD op dit moment draait komen er per run bij.
//
// `latest` alleen is niet genoeg. Productie draait op een `sha-`-tag, en zodra
// een main-build `latest` doorschuift houdt het draaiende image alleen die
// sha-tag over.
export const PROTECTED_TAGS = new Set(['latest']);

// Een untagged versie die jonger is dan dit blijft staan. Vangt de race tussen
// onze versions-listing en een push die tijdens de run binnenkomt: de children
// van die push zijn dan al wel gelist, maar de index met de tag nog niet.
export const DEFAULT_GRACE_HOURS = 24;

// Een getagde `sha-`-versie die nergens draait blijft nog zo lang staan. Dekt
// de race met een deploy die doorschuift tussen het opvragen van de draaiende
// lijst en het verwijderen, en houdt een terugrol naar de vorige build
// mogelijk.
export const DEFAULT_RETENTION_DAYS = 7;

const INDEX_MEDIA_TYPES = new Set([
  'application/vnd.oci.image.index.v1+json',
  'application/vnd.docker.distribution.manifest.list.v2+json',
]);

const MANIFEST_ACCEPT = [
  'application/vnd.oci.image.index.v1+json',
  'application/vnd.oci.image.manifest.v1+json',
  'application/vnd.docker.distribution.manifest.list.v2+json',
  'application/vnd.docker.distribution.manifest.v2+json',
].join(', ');

export class RateLimitError extends Error {
  constructor(message) {
    super(message);
    this.name = 'RateLimitError';
  }
}

export class HttpError extends Error {
  constructor(status, url, body) {
    super(`HTTP ${status} voor ${url}${body ? `: ${body}` : ''}`);
    this.name = 'HttpError';
    this.status = status;
  }
}

// --- pure logica (getest in ghcr-cleanup.test.mjs) ---------------------------

export const tagsOf = (version) => version?.metadata?.container?.tags ?? [];

/**
 * Leest uit het ZAD-antwoord welke image-tags er op dit moment draaien.
 *
 * Elke twijfel is hier een fout en geen lege verzameling: een lege lijst is
 * niet "er draait niets", het is ook wat een verlopen sleutel of een
 * foutpagina oplevert — en dan zou élke `sha-`-tag als ongebruikt gelden,
 * inclusief die van productie.
 *
 * De tag staat na de laatste `:` in het laatste padsegment. Op de hele URL zou
 * een registry met poort (`registry.internal:5000/o/p:sha-abc`) een
 * niet-bestaande "tag" opleveren, en geen match betekent hier: mag weg.
 */
export function runningTagsFrom(payload) {
  if (!payload || typeof payload !== 'object' || !Array.isArray(payload.deployments)) {
    throw new Error('het antwoord van ZAD bevat geen deployments-lijst');
  }

  const tags = new Set();
  for (const deployment of payload.deployments) {
    for (const component of deployment?.components ?? []) {
      const image = component?.image;
      if (!image) continue;
      const last = image.slice(image.lastIndexOf('/') + 1);
      if (last.includes('@')) {
        // Digest-gepind: de tag is niet af te leiden. Doorgaan zou de
        // bescherming stil uitschakelen voor precies dit image.
        throw new Error(
          `ZAD rapporteert een digest-gepinde image (${image}); dan is niet vast te stellen welke tag draait`,
        );
      }
      const colon = last.lastIndexOf(':');
      // Geen tag betekent latest, en die is sowieso beschermd.
      tags.add(colon === -1 ? 'latest' : last.slice(colon + 1));
    }
  }

  if (tags.size === 0) {
    throw new Error('ZAD gaf geen enkele draaiende image terug; zonder die lijst is niets veilig te verwijderen');
  }
  return tags;
}

export const isIndexMediaType = (mediaType) => INDEX_MEDIA_TYPES.has(mediaType);

/**
 * Herkent of een antwoord überhaupt een manifest is.
 *
 * Een 200 met een body die geen manifest is (een foutobject van een proxy, een
 * omgeleide loginpagina die toevallig JSON teruggeeft) levert anders stilletjes
 * "nul children" op — en daarmee worden de échte children van die getagde
 * versie als wees aangemerkt. Een index zonder children is legitiem
 * (`manifests: []`); een body zonder één van deze velden is dat niet.
 */
export const isManifestLike = (manifest) =>
  !!manifest &&
  typeof manifest === 'object' &&
  (Array.isArray(manifest.manifests) || Array.isArray(manifest.layers) || Array.isArray(manifest.fsLayers));

/**
 * Bouwt de verzameling digests waar een getagde versie naar wijst.
 *
 * `roots` zijn de digests van de getagde versies; `fetchManifest(digest)` levert
 * het geparste manifest of gooit. Een index kan in theorie naar een andere
 * index wijzen, dus de wandeling is transitief — maar alleen langs children
 * waarvan de descriptor al zegt dat het een index is. Dat kost geen extra
 * requests voor de image- en attestation-manifesten die buildx in de praktijk
 * produceert, en dekt de geneste variant toch af.
 *
 * Elk manifest dat we niet konden lezen laat de graaf onvolledig achter en komt
 * in `unresolved` of `missing`. De aanroeper mag dan niets verwijderen: elke
 * "wees" kan een child zijn van precies het manifest dat we niet konden lezen.
 * Zie `graphGaps`.
 */
export async function collectReferencedDigests({ roots, fetchManifest, concurrency = 8 }) {
  const referenced = new Set();
  const unresolved = [];
  const missing = [];
  const visited = new Set();
  const uniqueRoots = [...new Set(roots)];
  let frontier = uniqueRoots;
  // `mapLimit` breekt bij een fout alleen de runner af die struikelt; de andere
  // lopen hun deel van de frontier gewoon uit. Bij een rate limit is dat precies
  // verkeerd om: het besluit om te stoppen is dan al genomen, maar er vuren nog
  // honderden lookups af tegen de registry die net om rust vroeg — elk met zijn
  // eigen backoff, zodat de stap in zijn `timeout-minutes` loopt in plaats van
  // netjes af te sluiten met de melding. Vandaar deze gedeelde stopvlag: wie nog
  // niet begonnen was, begint niet meer.
  let rateLimited = null;

  while (frontier.length > 0) {
    const next = [];
    await mapLimit(frontier, concurrency, async (digest) => {
      if (rateLimited) return;
      if (visited.has(digest)) return;
      visited.add(digest);

      let manifest;
      try {
        manifest = await fetchManifest(digest);
      } catch (err) {
        if (err instanceof RateLimitError) {
          rateLimited = err;
          throw err;
        }
        // Een 404 wordt apart geteld omdat hij iets anders bétekent — de versie
        // is tussen listing en lookup verdwenen — maar hij blokkeert net zo
        // hard. Het is verleidelijk om hem af te doen als "weg is weg, dus die
        // beschermt niets meer", en voor één losse versie klopt dat ook. Alleen
        // geeft GHCR op een repo die niet bestaat exact dezelfde
        // `404 MANIFEST_UNKNOWN` als op een digest die niet bestaat. Eén
        // verkeerd registry-pad — hernoemd package, andere org, gewijzigde
        // auth — laat daarmee élke lookup 404 geven, houdt een lege
        // referentieverzameling over en zet alle nog levende children als
        // "wees" in de deletelijst. De eindcontrole houdt ze dan niet meer
        // tegen, want die vergelijkt met precies die lege verzameling.
        if (err instanceof HttpError && err.status === 404) {
          missing.push(digest);
          return;
        }
        unresolved.push({ digest, error: err?.message ?? String(err) });
        return;
      }

      if (!isManifestLike(manifest)) {
        unresolved.push({ digest, error: 'antwoord is geen manifest' });
        return;
      }

      for (const child of manifest.manifests ?? []) {
        if (!child?.digest) continue;
        referenced.add(child.digest);
        if (isIndexMediaType(child.mediaType) && !visited.has(child.digest)) {
          next.push(child.digest);
        }
      }
    });
    frontier = next;
  }

  return { referenced, unresolved, missing, rootCount: uniqueRoots.length };
}

/**
 * Alle redenen waarom de referentiegraaf van een package niet te vertrouwen is.
 * Zolang deze lijst niet leeg is mag er geen enkele wees verwijderd worden.
 *
 * Dit is de faalveilige kern van het script en staat daarom op één plek: elke
 * manier waarop de graaf onvolledig kan zijn hoort hier bij te komen, niet
 * verspreid over de aanroepers.
 */
export function graphGaps({ rootCount = 0, referenced, unresolved = [], missing = [] }) {
  const gaps = [];
  if (unresolved.length > 0) {
    gaps.push(`${unresolved.length} manifest-lookup(s) mislukt (bv. ${unresolved[0].digest} — ${unresolved[0].error})`);
  }
  if (missing.length > 0) {
    gaps.push(`${missing.length} getagde versie(s) gaven 404 op hun manifest (bv. ${missing[0]})`);
  }
  // Een package met getagde versies dat geen enkele child oplevert is geen
  // opruimklus maar een storing: zonder dit hek is de eindcontrole vleugellam,
  // want die toetst de deletelijst tegen een lege verzameling.
  if (rootCount > 0 && referenced.size === 0) {
    gaps.push(`geen enkele van de ${rootCount} getagde versies leverde een child op`);
  }
  return gaps;
}

/**
 * Bepaalt wat er voor één package weg mag.
 *
 * Drie soorten kandidaten: getagde `pr-*`-versies van gesloten PR's, getagde
 * versies waarvan élke tag de `sha-`-vorm heeft en die niemand meer draait, en
 * untagged versies waar geen getagde index naar wijst (de wezen). Is de
 * referentiegraaf onvolledig (`gaps` niet leeg), dan vervallen de wezen — de
 * twee getagde takken hangen niet van de graaf af en blijven wel werken.
 *
 * `protectedTags` is de bodem (`latest`) plus wat ZAD op dit moment draait.
 * Een versie met zo'n tag komt in geen enkele tak voor, wat er verder ook op
 * zit.
 */
export function planPackage({
  packageName,
  versions,
  openPrs,
  referenced,
  protectedTags = PROTECTED_TAGS,
  gaps = [],
  now = Date.now(),
  graceMs = DEFAULT_GRACE_HOURS * 3600 * 1000,
  retentionMs = DEFAULT_RETENTION_DAYS * 24 * 3600 * 1000,
}) {
  const openPrSet = new Set([...openPrs].map(String));
  const tagged = [];
  const untagged = [];
  for (const version of versions) {
    (tagsOf(version).length > 0 ? tagged : untagged).push(version);
  }

  const protectedVersions = tagged.filter((v) => tagsOf(v).some((t) => protectedTags.has(t)));
  const protectedIds = new Set(protectedVersions.map((v) => v.id));

  // Een getagde versie die zelf een child van een andere index is, hoort bij
  // die index en niet bij deze opruimer. In de praktijk komt dat niet voor —
  // buildx tagt alleen de index — maar als het gebeurt telt de graaf zwaarder
  // dan de tagvorm, en dan hoeft de eindcontrole hieronder de hele run niet af
  // te breken over iets wat hier af te vangen is.
  const deletable = (version) => !protectedIds.has(version.id) && !referenced.has(version.name);

  // Een versie telt als stale PR-image wanneer er minstens één `pr-*`-tag op
  // zit en géén daarvan bij een open PR hoort. Verwijderen haalt ook de
  // meeliftende `sha-*`-tag weg; dat was al zo en is ook de bedoeling.
  const stalePr = tagged.filter((version) => {
    if (!deletable(version)) return false;
    const prNumbers = tagsOf(version)
      .filter((t) => t.startsWith('pr-'))
      .map((t) => t.slice('pr-'.length));
    return prNumbers.length > 0 && prNumbers.every((n) => !openPrSet.has(n));
  });

  // De sha-tak. Alléén afgaan op de tagvorm zou het image onder de draaiende
  // deployment vandaan halen: `protectedTags` bevat daarom de tags die ZAD nu
  // draait, en de leeftijdsgrens dekt de race met een deploy die doorschuift
  // terwijl deze run bezig is.
  const staleSha = tagged.filter((version) => {
    if (!deletable(version)) return false;
    if (!tagsOf(version).every((t) => t.startsWith('sha-'))) return false;
    const age = now - Date.parse(version.updated_at ?? version.created_at);
    // Een onleesbare datum slaat de versie over; onbekend is geen vrijbrief.
    return Number.isFinite(age) && age >= retentionMs;
  });

  const referencedUntagged = untagged.filter((v) => referenced.has(v.name));
  const candidates = untagged.filter((v) => !referenced.has(v.name));

  const orphans = [];
  const tooRecent = [];
  for (const version of candidates) {
    const age = now - Date.parse(version.created_at);
    // Een onleesbare datum telt als "te vers": onbekend is geen vrijbrief.
    (Number.isFinite(age) && age >= graceMs ? orphans : tooRecent).push(version);
  }

  const blocked = gaps.length > 0;
  const taggedDeletion = (reason) => (version) => ({
    id: version.id,
    digest: version.name,
    tags: tagsOf(version),
    reason,
  });
  const deletions = [
    ...stalePr.map(taggedDeletion('stale-pr')),
    ...staleSha.map(taggedDeletion('stale-sha')),
    ...(blocked
      ? []
      : orphans.map((version) => ({
          id: version.id,
          digest: version.name,
          tags: [],
          reason: 'orphan',
        }))),
  ];

  return {
    packageName,
    blocked,
    gaps,
    counts: {
      total: versions.length,
      tagged: tagged.length,
      untagged: untagged.length,
      referencedDigests: referenced.size,
      referencedUntagged: referencedUntagged.length,
      orphaned: orphans.length,
      skippedTooRecent: tooRecent.length,
      stalePr: stalePr.length,
      staleSha: staleSha.length,
      protectedByTag: protectedVersions.length,
    },
    deletions,
  };
}

/**
 * Laatste hek voor de DELETE's: geen enkele digest in de deletelijst mag in de
 * referentieverzameling zitten. Gooit met de overtreders, zodat een run die
 * hier struikelt niets verwijdert in plaats van het verkeerde.
 */
export function assertNoReferencedDeletions(deletions, referenced) {
  const violations = deletions.filter((d) => referenced.has(d.digest));
  if (violations.length > 0) {
    throw new Error(
      `veiligheidscontrole gefaald: ${violations.length} gerefereerde digest(s) in de deletelijst ` +
        `(${violations.slice(0, 5).map((v) => v.digest).join(', ')})`,
    );
  }
  return true;
}

export function formatSummary(results, { dryRun, runningTags = [] }) {
  const lines = [];
  lines.push(`## GHCR-opruiming${dryRun ? ' (dry-run)' : ''}`, '');
  // Welke tags beschermd waren hoort in de samenvatting, want dat is het enige
  // wat verklaart waarom een `sha-`-versie bleef staan of juist niet.
  const running = [...runningTags].sort();
  if (running.length > 0) {
    lines.push(`Draait volgens ZAD (${running.length}): ${running.map((t) => `\`${t}\``).join(', ')}.`, '');
  }
  lines.push(
    '| package | versies | getagd | untagged | gerefereerd | in gebruik | wees | te vers | stale pr-* | stale sha-* | te verwijderen | verwijderd | fouten |',
  );
  lines.push('|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|');
  for (const result of results) {
    if (result.error) {
      lines.push(`| ${result.packageName} | overgeslagen: ${result.error} | | | | | | | | | | | |`);
      continue;
    }
    const c = result.counts;
    lines.push(
      `| ${result.packageName} | ${c.total} | ${c.tagged} | ${c.untagged} | ${c.referencedUntagged} | ` +
        `${c.protectedByTag} | ${c.orphaned} | ${c.skippedTooRecent} | ${c.stalePr} | ${c.staleSha} | ` +
        `${result.deletions?.length ?? 0} | ${dryRun ? '—' : (result.deleted ?? 0)} | ` +
        `${result.failures?.length ?? 0} |`,
    );
  }
  lines.push('');

  // Het bewijs dat de opruimer de referentiegraaf respecteert hoort in de
  // samenvatting zelf te staan, niet alleen in de exitcode: per package het
  // aantal gerefereerde digests naast het gemeten aantal dat in de deletelijst
  // zit. Dat getal hoort nul te zijn, maar het wordt geteld en niet beloofd —
  // een samenvatting die "0" opschrijft omdat dat de bedoeling is, verzwijgt
  // precies de run die je had willen zien.
  const planned = results.filter((r) => !r.error);
  if (planned.length > 0) {
    const totalReferenced = planned.reduce((sum, r) => sum + r.counts.referencedDigests, 0);
    const totalDeletions = planned.reduce((sum, r) => sum + (r.deletions?.length ?? 0), 0);
    const totalReferencedInDeletions = planned.reduce((sum, r) => sum + (r.referencedInDeletions ?? 0), 0);
    lines.push(
      `${totalReferenced} gerefereerde digest(s) in kaart gebracht; ` +
        `daarvan staan er **${totalReferencedInDeletions}** in de ${totalDeletions} geplande verwijdering(en).`,
      '',
    );
  }

  const blocked = results.filter((r) => r.blocked);
  if (blocked.length > 0) {
    lines.push('### Onvolledige referentiegraaf', '');
    lines.push('Voor deze packages is de referentiegraaf niet compleet. Er zijn geen wezen verwijderd:', '');
    for (const result of blocked) {
      lines.push(`- **${result.packageName}**: ${(result.gaps ?? []).join('; ')}`);
    }
    lines.push('');
  }

  const withFailures = results.filter((r) => (r.failures?.length ?? 0) > 0);
  if (withFailures.length > 0) {
    lines.push('### Mislukte verwijderingen', '');
    for (const result of withFailures) {
      for (const failure of result.failures) {
        lines.push(`- \`${result.packageName}\` versie ${failure.id} (${failure.digest}): ${failure.error}`);
      }
    }
    lines.push('');
  }

  return lines.join('\n');
}

// --- HTTP ---------------------------------------------------------------

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export async function mapLimit(items, limit, worker) {
  const results = new Array(items.length);
  let index = 0;
  const runners = Array.from({ length: Math.max(1, Math.min(limit, items.length)) }, async () => {
    while (index < items.length) {
      const current = index++;
      results[current] = await worker(items[current], current);
    }
  });
  await Promise.all(runners);
  return results;
}

// Een enkele call mag nooit oneindig blijven hangen: deze run doet er duizenden
// en draait onbewaakt in een nachtelijke pipeline.
const REQUEST_TIMEOUT_MS = 30_000;

// Bovengrens op één enkele `Retry-After`-pauze.
export const MAX_RETRY_AFTER_SECONDS = 60;

/**
 * Herkent of een antwoord een rate limit is, en zo ja hoe lang we moeten
 * wachten. Geeft `null` als het er geen is.
 *
 * Drie signalen, en het derde is het makkelijkst te missen:
 *
 *   * `429` — altijd een limiet.
 *   * `403` met `x-ratelimit-remaining: 0` — het primaire quotum is op.
 *   * `403` met een `Retry-After` — GitHub's *secondary* (abuse) limit. Die
 *     laat `x-ratelimit-remaining` juist met rust, want het primaire quotum is
 *     niet op. Het is bovendien precies de limiet die toeslaat op wat dit
 *     script doet: duizenden lookups achter elkaar. Alleen op
 *     `x-ratelimit-remaining` afgaan slaat dan de backoff over en negeert de
 *     `Retry-After` op het moment dat je hem nodig hebt.
 */
export function rateLimitOf({ status, headers }) {
  if (status !== 429 && status !== 403) return null;
  const retryAfter = Number(headers.get('retry-after'));
  const hasRetryAfter = Number.isFinite(retryAfter) && retryAfter > 0;
  const exhausted = headers.get('x-ratelimit-remaining') === '0';
  if (status !== 429 && !exhausted && !hasRetryAfter) return null;
  return { retryAfterSeconds: hasRetryAfter ? retryAfter : 0 };
}

// Haalt een URL op met backoff op 429 en 5xx. Een 429 die na alle pogingen
// blijft staan is geen incident maar een muur: dan stopt de run liever dan dat
// hij met een halve graaf verder rekent.
async function request(url, { method = 'GET', headers = {}, attempts = 5 } = {}) {
  let lastError;
  for (let attempt = 0; attempt < attempts; attempt++) {
    if (attempt > 0) await sleep(Math.min(30_000, 1000 * 2 ** (attempt - 1)));

    let response;
    try {
      response = await fetch(url, { method, headers, signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS) });
    } catch (err) {
      lastError = err;
      continue;
    }

    const limit = rateLimitOf(response);
    if (limit) {
      if (attempt === attempts - 1) {
        throw new RateLimitError(`rate limit bereikt op ${url} (status ${response.status})`);
      }
      // Wachten wat GitHub vraagt, maar niet ongelimiteerd: de stap heeft een
      // eigen tijdslimiet en de run stopt liever netjes met een RateLimitError
      // dan dat hij zijn budget in één sleep opmaakt.
      await sleep(Math.min(limit.retryAfterSeconds, MAX_RETRY_AFTER_SECONDS) * 1000);
      continue;
    }

    if (response.status >= 500) {
      lastError = new HttpError(response.status, url, (await response.text()).slice(0, 200));
      continue;
    }

    return response;
  }
  throw lastError ?? new Error(`geen antwoord van ${url}`);
}

function createClient({ token, org, zad }) {
  const apiHeaders = {
    accept: 'application/vnd.github+json',
    authorization: `Bearer ${token}`,
    'x-github-api-version': '2022-11-28',
    'user-agent': 'regelrecht-ghcr-cleanup',
  };
  // De registry-API verwacht hetzelfde token, maar base64 als basic-credential
  // in een bearer verpakt. Zonder die omweg krijg je een 401.
  const registryBearer = Buffer.from(`x:${token}`).toString('base64');
  const registryOrg = org.toLowerCase();

  const api = async (path, init) => {
    const response = await request(`https://api.github.com${path}`, { ...init, headers: apiHeaders });
    if (!response.ok) {
      throw new HttpError(response.status, path, (await response.text()).slice(0, 200));
    }
    return response;
  };

  return {
    async listPackageVersions(packageName) {
      // De listing loopt over tientallen pagina's en staat nieuw-eerst. Komt er
      // tijdens het pagineren een push binnen, dan schuift alles een plek op en
      // duikt dezelfde versie op twee pagina's op. Ontdubbelen op id houdt de
      // tellingen eerlijk en voorkomt een tweede DELETE op een versie die al
      // weg is.
      const byId = new Map();
      let path = `/orgs/${org}/packages/container/${encodeURIComponent(packageName)}/versions?per_page=100`;
      while (path) {
        const response = await api(path);
        for (const version of await response.json()) byId.set(version.id, version);
        path = nextLink(response.headers.get('link'));
      }
      return [...byId.values()];
    },

    async listOpenPullRequests(repo) {
      const numbers = [];
      let path = `/repos/${repo}/pulls?state=open&per_page=100`;
      while (path) {
        const response = await api(path);
        for (const pr of await response.json()) numbers.push(pr.number);
        path = nextLink(response.headers.get('link'));
      }
      return numbers;
    },

    async fetchManifest(packageName, reference) {
      const url = `https://ghcr.io/v2/${registryOrg}/${packageName}/manifests/${reference}`;
      const response = await request(url, {
        headers: { authorization: `Bearer ${registryBearer}`, accept: MANIFEST_ACCEPT },
      });
      if (!response.ok) {
        throw new HttpError(response.status, url, (await response.text()).slice(0, 200));
      }
      return response.json();
    },

    async deleteVersion(packageName, versionId) {
      await api(`/orgs/${org}/packages/container/${encodeURIComponent(packageName)}/versions/${versionId}`, {
        method: 'DELETE',
      });
    },

    // Wat ZAD op dit moment draait. Niet via `gh`, maar rechtstreeks: dezelfde
    // endpoint die de andere opruimscripts in scheduled-cleanup.yml gebruiken.
    async fetchZadDeployments() {
      const url = `${zad.apiBase.replace(/\/$/, '')}/v2/projects/${encodeURIComponent(zad.project)}/deployments`;
      const response = await request(url, {
        headers: { 'x-api-key': zad.apiKey, accept: 'application/json', 'user-agent': 'regelrecht-ghcr-cleanup' },
      });
      if (!response.ok) {
        throw new HttpError(response.status, url, (await response.text()).slice(0, 200));
      }
      return response.json();
    },
  };
}

export function nextLink(linkHeader) {
  if (!linkHeader) return null;
  for (const part of linkHeader.split(',')) {
    const match = part.match(/<([^>]+)>\s*;\s*rel="next"/);
    if (match) return match[1].replace('https://api.github.com', '');
  }
  return null;
}

// --- CLI ----------------------------------------------------------------

export function parseArgs(argv) {
  const options = {
    dryRun: false,
    packages: PACKAGES,
    graceHours: DEFAULT_GRACE_HOURS,
    retentionDays: DEFAULT_RETENTION_DAYS,
    concurrency: 8,
  };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--dry-run') options.dryRun = true;
    else if (arg === '--packages') options.packages = argv[++i].split(',').map((s) => s.trim()).filter(Boolean);
    else if (arg === '--grace-hours') options.graceHours = Number(argv[++i]);
    else if (arg === '--retention-days') options.retentionDays = Number(argv[++i]);
    else if (arg === '--concurrency') options.concurrency = Number(argv[++i]);
    else throw new Error(`onbekend argument: ${arg}`);
  }
  if (!Number.isFinite(options.graceHours) || options.graceHours < 0) {
    throw new Error('--grace-hours verwacht een niet-negatief getal');
  }
  if (!Number.isFinite(options.retentionDays) || options.retentionDays < 0) {
    throw new Error('--retention-days verwacht een niet-negatief getal');
  }
  if (!Number.isInteger(options.concurrency) || options.concurrency < 1) {
    throw new Error('--concurrency verwacht een positief geheel getal');
  }
  return options;
}

const log = (message) => process.stdout.write(`${message}\n`);
const annotate = (level, message) => log(`::${level}::${message.replace(/\n/g, '%0A')}`);

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const dryRun = options.dryRun || process.env.DRY_RUN === 'true';
  const token = process.env.GH_TOKEN;
  const org = process.env.ORG || process.env.GITHUB_REPOSITORY_OWNER;
  const repo = process.env.GITHUB_REPOSITORY || (org ? `${org}/regelrecht` : null);

  const zad = {
    apiBase: process.env.ZAD_API_BASE,
    apiKey: process.env.ZAD_API_KEY,
    project: process.env.ZAD_PROJECT,
  };

  if (!token) throw new Error('GH_TOKEN ontbreekt');
  if (!org) throw new Error('ORG of GITHUB_REPOSITORY_OWNER ontbreekt');
  // Geen stilzwijgende terugval op "dan maar zonder": zonder deze drie is niet
  // vast te stellen welke `sha-`-tag productie draait, en dan mag er niets weg.
  for (const [name, value] of [
    ['ZAD_API_BASE', zad.apiBase],
    ['ZAD_API_KEY', zad.apiKey],
    ['ZAD_PROJECT', zad.project],
  ]) {
    if (!value) throw new Error(`${name} ontbreekt; zonder de draaiende deployments wordt er niets verwijderd`);
  }

  const client = createClient({ token, org, zad });
  const graceMs = options.graceHours * 3600 * 1000;
  const retentionMs = options.retentionDays * 24 * 3600 * 1000;

  log(`Opruiming van ${options.packages.length} package(s) in ${org}${dryRun ? ' — dry-run' : ''}`);

  // Eerst vaststellen wat er draait, dan pas kijken wat er weg mag. Faalt dit,
  // dan stopt de run hier — vóór de eerste DELETE.
  let runningTags;
  try {
    runningTags = runningTagsFrom(await client.fetchZadDeployments());
  } catch (err) {
    throw new Error(`kon niet vaststellen wat er draait: ${err.message}; er wordt niets verwijderd`);
  }
  const protectedTags = new Set([...PROTECTED_TAGS, ...runningTags]);
  log(`In gebruik volgens ZAD (${runningTags.size}): ${[...runningTags].sort().join(' ')}`);

  const openPrs = await client.listOpenPullRequests(repo);
  log(`Open PR's: ${openPrs.length}`);

  const results = [];
  let sawDeleteFailure = false;
  let sawPackageError = false;

  // Wat er ook misgaat, de samenvatting van wat er tot dat moment is gebeurd
  // moet eruit komen: bij destructief werk is "welke DELETE's zijn al gedaan"
  // precies wat je wilt weten als een run halverwege stopt.
  try {
    packages: for (const packageName of options.packages) {
      log(`\n--- ${packageName}`);
      let versions;
      let graph;
      try {
        versions = await client.listPackageVersions(packageName);
        const roots = versions.filter((v) => tagsOf(v).length > 0).map((v) => v.name);
        log(`  ${versions.length} versies, ${roots.length} getagd — manifesten ophalen...`);
        graph = await collectReferencedDigests({
          roots,
          concurrency: options.concurrency,
          fetchManifest: (digest) => client.fetchManifest(packageName, digest),
        });
      } catch (err) {
        results.push({ packageName, error: err.message });
        sawPackageError = true;
        if (err instanceof RateLimitError) {
          annotate('error', `Rate limit tijdens ${packageName}; run gestopt zonder verder op te ruimen: ${err.message}`);
          break packages;
        }
        annotate('error', `${packageName} overgeslagen: ${err.message}`);
        continue;
      }

      const gaps = graphGaps(graph);
      const plan = planPackage({
        packageName,
        versions,
        openPrs,
        referenced: graph.referenced,
        protectedTags,
        gaps,
        graceMs,
        retentionMs,
      });
      // Geteld, niet aangenomen: dit is het getal waarmee de samenvatting
      // criterium 4 hardmaakt.
      const referencedInDeletions = plan.deletions.filter((d) => graph.referenced.has(d.digest)).length;
      const result = {
        ...plan,
        unresolved: graph.unresolved,
        missing: graph.missing,
        referencedInDeletions,
        deleted: 0,
        failures: [],
      };
      results.push(result);

      const c = plan.counts;
      log(
        `  totaal ${c.total} | getagd ${c.tagged} | untagged ${c.untagged} | ` +
          `gerefereerd ${c.referencedUntagged} | in gebruik ${c.protectedByTag} | wees ${c.orphaned} | ` +
          `te vers ${c.skippedTooRecent} | stale pr-* ${c.stalePr} | stale sha-* ${c.staleSha} | ` +
          `te verwijderen ${plan.deletions.length}`,
      );
      if (plan.blocked) {
        annotate(
          'warning',
          `${packageName}: referentiegraaf onvolledig (${gaps.join('; ')}) — geen wezen verwijderd deze ronde`,
        );
        for (const item of graph.unresolved.slice(0, 5)) log(`    onopgelost: ${item.digest} — ${item.error}`);
        for (const digest of graph.missing.slice(0, 5)) log(`    404 op getagd manifest: ${digest}`);
      }

      assertNoReferencedDeletions(plan.deletions, graph.referenced);

      for (const deletion of plan.deletions) {
        const label = `${packageName} ${deletion.digest}${deletion.tags.length ? ` [${deletion.tags.join(', ')}]` : ''} (${deletion.reason})`;
        if (dryRun) {
          log(`  [dry-run] zou verwijderen: ${label}`);
          continue;
        }
        try {
          await client.deleteVersion(packageName, deletion.id);
          result.deleted++;
          log(`  verwijderd: ${label}`);
        } catch (err) {
          // Een losse mislukte DELETE mag de run niet laten falen — die wordt
          // gerapporteerd en volgende ronde opnieuw geprobeerd. Een rate limit
          // is iets anders: dan is elke volgende call ook kansloos en stoppen
          // we liever dan half opgeruimd door te denderen.
          sawDeleteFailure = true;
          result.failures.push({ id: deletion.id, digest: deletion.digest, error: err.message });
          annotate('warning', `DELETE mislukt voor ${label}: ${err.message}`);
          if (err instanceof RateLimitError) {
            annotate('error', `Rate limit tijdens het verwijderen in ${packageName}; run gestopt: ${err.message}`);
            sawPackageError = true;
            break packages;
          }
        }
      }
    }
  } finally {
    // Wat er ook misgaat, de samenvatting van wat er tot dat moment gebeurd is
    // moet eruit komen: bij destructief werk is "welke DELETE's zijn al
    // gedaan" precies wat je wilt weten als een run halverwege stopt.
    log('');
    log(formatSummary(results, { dryRun, runningTags }));
    writeSummary(results, { dryRun, runningTags });
  }

  if (sawPackageError) process.exitCode = 1;
  if (sawDeleteFailure) log('Er waren mislukte verwijderingen; zie de samenvatting hierboven.');
}

function writeSummary(results, options) {
  const target = process.env.GITHUB_STEP_SUMMARY;
  if (!target) return;
  appendFileSync(target, `${formatSummary(results, options)}\n`);
}

const invokedDirectly =
  process.argv[1] && realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url));
if (invokedDirectly) {
  main().catch((err) => {
    annotate('error', `GHCR-opruiming gestopt: ${err.message}`);
    process.exitCode = 1;
  });
}
