import { defineCollection, z } from 'astro:content';
import { glob } from 'astro/loaders';
import {
  ONDERZOEK_IDS,
  BOUW_IDS,
  BELEGGING_IDS,
  PRIORITEIT_IDS,
  OMVANG_IDS,
  CATEGORIE_IDS,
  CAPABILITY_IDS,
} from '~/lib/roadmap';
import { RFC_TOPIC_IDS } from '~/lib/rfc-topics';

const docs = defineCollection({
  loader: glob({ pattern: '**/*.{md,mdx}', base: 'src/content/docs' }),
  schema: z.object({
    title: z.string().optional(),
    description: z.string().optional(),
    // Per-page language override. Docs default to English; a Dutch page (e.g.
    // the accessibility statement) sets `lang: nl` so its <html lang> — and
    // thus screen-reader pronunciation — matches the content.
    lang: z.enum(['en', 'nl']).optional(),
  }),
});

const rfcs = defineCollection({
  loader: glob({ pattern: 'rfc-*.md', base: 'src/content/rfcs' }),
  schema: z
    .object({
      title: z.string().optional(),
      description: z.string().optional(),
      // RFC metadata, in frontmatter so it is structured data rather than a
      // bold-labelled preamble parsed out of the body. Both status and
      // implementation are required enums: every RFC carries both (so an absent
      // implementation tag never reads as "unknown"), and a typo fails the build
      // here rather than rendering a silent grey "Unknown" badge.
      //
      // `Reserved` is the exception, and the only status that is not a place in
      // the decision lifecycle: it is a placeholder holding a number that an
      // open pull request has already claimed, so the next author does not take
      // it a second time. It carries no design, so `implementation` is not just
      // optional but forbidden — see the superRefine below.
      //
      // What makes a file a placeholder is `reserved_by`, not the status value.
      // A placeholder whose pull request closes unmerged goes to `Rejected` and
      // keeps its `reserved_by`: the number stays spent, and the link is the
      // record of what spent it. Keying the rule on `status === 'Reserved'`
      // would make that documented transition a build error.
      status: z.enum([
        'Draft',
        'Proposed',
        'Accepted',
        'Rejected',
        'Superseded',
        'Reserved',
      ]),
      implementation: z
        .enum(['Implemented', 'Partially implemented', 'Not implemented'])
        .optional(),
      // The pull request that holds a reserved number. Required on (and only
      // on) a Reserved RFC: a reservation nobody can trace back to a branch is
      // a number blocked forever by an author nobody can find.
      reserved_by: z.string().url().optional(),
      // Stored as a 'YYYY-MM-DD' string rather than z.date() so it round-trips
      // through the build without timezone shifts and renders verbatim.
      date: z.string().optional(),
      authors: z.array(z.string()).optional(),
      depends_on: z.array(z.string()).optional(),
      // Sidebar label; falls back to the stripped title when absent.
      short_title: z.string().optional(),
      // The group the RFC index files it under (lib/rfc-topics.ts). Required
      // on every RFC that is not a placeholder, like `implementation`.
      topic: z.enum(RFC_TOPIC_IDS).optional(),
    })
    // Making `implementation` optional above would, on its own, let any RFC
    // drop it and render a silent gap where every sibling shows a tag. It is
    // therefore tied to `reserved_by`: a placeholder omits it, a real RFC
    // states it. Both directions are checked, because a placeholder that kept
    // claiming "Not implemented" would read as a design decision that was
    // never written down.
    //
    // A placeholder is any file carrying `reserved_by`, which covers both a
    // live reservation (`Reserved`) and a spent one whose pull request closed
    // unmerged (`Rejected`). `Reserved` additionally requires the link, since
    // a live reservation nobody can trace blocks the number forever.
    .superRefine((data, ctx) => {
      const isPlaceholder = data.reserved_by !== undefined;

      if (data.status === 'Reserved' && !data.reserved_by) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ['reserved_by'],
          message:
            'A Reserved RFC must name the pull request that claimed the number, so the reservation can be traced and released.',
        });
      }

      if (isPlaceholder) {
        if (data.implementation !== undefined) {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            path: ['implementation'],
            message:
              'A placeholder holds a number, not a design, so it carries no implementation state. Remove the field.',
          });
        }
        if (data.status !== 'Reserved' && data.status !== 'Rejected') {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            path: ['status'],
            message:
              "A file with `reserved_by` is a placeholder, so its status is 'Reserved' while its pull request is open, or 'Rejected' once that pull request closed unmerged. Any other status means the real RFC landed, and then `reserved_by` should go.",
          });
        }
      } else {
        if (data.implementation === undefined) {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            path: ['implementation'],
            message:
              'Every RFC that is not a placeholder states whether it is built, so an absent tag never reads as "unknown".',
          });
        }
        if (data.topic === undefined) {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            path: ['topic'],
            message:
              'Every RFC that is not a placeholder names its topic, so the RFC index can group it. The topics are listed in src/lib/rfc-topics.ts.',
          });
        }
      }
    }),
});

/*
 * Werkpakketten of the roadmap (/roadmap), one markdown file per werkpakket.
 *
 * The files carry frontmatter only. The app that produced them regenerated a
 * markdown body from these same fields on every save; nothing renders that
 * body here, so it was dropped rather than kept as a second copy free to
 * drift. `toelichting` is the prose field, rendered by roadmap-markdown.ts.
 *
 * The enums are built from the id lists in lib/roadmap.ts, which is also what
 * the pages render labels from, so a value the schema accepts always has a
 * label. Repeating the ids here instead would let a value validate while
 * getPrioriteit() returns undefined, and the card would render no tag at all
 * for a value that was set. Empty strings are allowed everywhere they occur
 * in practice: most
 * werkpakketten have no prioriteit, omvang, categorie or capability yet, and
 * an absent value is written as '' rather than omitted.
 *
 * faseId and disciplineId stay plain strings: they point into
 * src/data/roadmap-config.json, which a per-entry schema cannot see. That
 * check, and the samenhangIds one, live in assertReferencesResolve().
 */

/*
 * A werkpakket id is a slug: lowercase, digits, single hyphens between parts.
 *
 * It replaced a UUID. The id is the filename, the URL and what samenhangIds
 * point at, and a UUID made all three unreadable — a list of three
 * samenhangIds carried no information about the relation it recorded. The slug
 * is derived from the titel once and then stays put; it is deliberately not
 * kept in sync with the titel, because a reference that moves when a title is
 * edited is exactly what makes a derived slug unusable as an identity.
 *
 * The pattern rejects uppercase rather than tolerating it. The filename must
 * equal the id (assertReferencesResolve), that comparison is literal, and
 * macOS is case-insensitive: a capital would look right locally and fail the
 * build elsewhere.
 */
const SLUG = /^[a-z0-9]+(-[a-z0-9]+)*$/;
const slug = () =>
  z
    .string()
    .regex(SLUG, 'moet een slug zijn: kleine letters, cijfers en koppeltekens');

/*
 * De velden van een onderzoeksvraag in objectvorm.
 *
 * `id` is een slug, uniek over de hele roadmap, deelvragen meegerekend
 * (assertOnderzoeksvragen() in lib/roadmap.ts). Hij is waar `verwant` naar
 * wijst en wat het anker op de pagina's is, dus hij verandert niet meer als
 * de vraag geherformuleerd wordt; dezelfde reden als bij het id van een
 * werkpakket. Optioneel: een vraag zonder verwijzingen heeft hem niet nodig.
 *
 * `paper` is een sectie-anker uit het position paper, zonder '#';
 * assertPaperSections() controleert dat hij bestaat.
 *
 * `status` deelt het vocabulaire met `onderzoek` op het werkpakket: open,
 * loopt, beantwoord, of '' voor "niet bepaald". Hij wordt niet afgeleid uit
 * de deelvragen. Een vraag kan beantwoord zijn terwijl een deelvraag nog
 * open staat (het antwoord maakte hem onbelangrijk), en andersom; of de
 * hoofdvraag af is, is een oordeel over het geheel, zoals `klaar` bij de
 * belegging.
 *
 * `doel` zegt wat beantwoorden moet opleveren: een notitie, een prototype,
 * een besluit. Vrije tekst, mag leeg.
 *
 * `verwant` schrijf je één kant op; de pagina's lezen beide kanten, zoals
 * bij afhankelijkVan. Een vraag met `verwant` heeft een eigen `id` nodig,
 * anders kan de andere kant niet terugwijzen; assertOnderzoeksvragen() valt
 * daarop.
 *
 * `.strict()` op de objecten die deze velden gebruiken: een verschreven
 * sleutel (`deelvraag:`, `papersectie:`) zou anders stil verdwijnen, en dat
 * is precies de fout die niemand terugvindt.
 */
const vraagVelden = {
  vraag: z.string().min(1),
  id: slug().optional(),
  paper: z.string().min(1).optional(),
  status: z.enum(ONDERZOEK_IDS).or(z.literal('')).default(''),
  doel: z.string().default(''),
  verwant: z.array(slug()).default([]),
};

/*
 * Een user story onder een onderzoeksvraag: het niveau waarop het team
 * prioriteert, en wat een casus overdraagbaar maakt tussen analisten.
 *
 * Een story is een titel plus een paragraaf, in de taal van de gebruiker, en
 * niet meer: geen punten, geen inschatting, geen namen. Taken horen er wel
 * onder, maar die staan in Bouwmeester en niet op de publieke roadmap; de
 * roadmap zegt wat er moet kunnen, de backlog zegt wie wat doet.
 *
 * `verhaal` is die ene paragraaf ("Als analist wil ik … zodat …"). Mag leeg:
 * een story begint met een titel en een plek, de rest volgt.
 *
 * `stand` deelt het vocabulaire van de belegging van een werkpakket (vrij,
 * opgepakt, klaar) en niet dat van `status` op de vraag: een story is werk
 * dat iemand oppakt en afrondt, geen vraag die beantwoord raakt. Leeg is
 * "volgt de vraag": een story zonder eigen stand staat in de lane van zijn
 * vraag, zoals een deelvraag dat doet (storyLane() in lib/roadmap.ts).
 *
 * `acceptatiecriteria` horen bij déze story en zeggen wat de oplevering
 * specifiek moet doen. Ze zijn iets anders dan de definition of done, die
 * voor elke oplevering geldt en daarom niet per story herhaald wordt.
 *
 * `voorstel` markeert een story die nog niet door het team is vastgesteld:
 * een suggestie om de vraag concreet te maken. De pagina's tonen hem met een
 * tag "Voorstel", zodat een lezer ziet wat er al staat en wat nog een idee
 * is. Een vastgestelde story heeft het veld niet.
 *
 * De volgorde in de lijst is de prioriteit: de bovenste eerst. Geen apart
 * veld, want een getal naast een lijst raakt uit de pas met die lijst.
 */
const storyVelden = {
  titel: z.string().min(1),
  id: slug().optional(),
  verhaal: z.string().default(''),
  stand: z.enum(BELEGGING_IDS).or(z.literal('')).default(''),
  acceptatiecriteria: z.array(z.string().min(1)).default([]),
  voorstel: z.boolean().default(false),
};

const werkpakketten = defineCollection({
  loader: glob({
    pattern: '*.md',
    base: 'src/content/roadmap/werkpakketten',
  }),
  schema: z.object({
    id: slug(),
    titel: z.string(),
    faseId: z.string(),
    disciplineId: z.string(),
    prioriteit: z.enum(PRIORITEIT_IDS).or(z.literal('')),
    omvang: z.enum(OMVANG_IDS).or(z.literal('')),
    categorie: z.enum(CATEGORIE_IDS).or(z.literal('')),
    capability: z.enum(CAPABILITY_IDS).or(z.literal('')),
    capaciteit: z.string().default(''),
    toelichting: z.string().default(''),
    // Required, not defaulted: `volgorde` places the werkpakket inside its
    // matrix cell, and a default of 0 would silently sort a file that forgot
    // it to the front rather than say so.
    volgorde: z.number(),
    /*
     * Een onderzoeksvraag is een gewone string, of een object met de velden
     * van `vraagVelden` hierboven. De stringvorm blijft: een vraag waar niets
     * over te zeggen valt dan de vraag zelf hoort geen vijf lege velden te
     * dragen, en zo veranderen alleen de bestanden die iets toevoegen.
     * Genormaliseerd voor de pagina's door onderzoeksvraagLijst().
     *
     * `deelvragen` gaat één niveau diep: een deelvraag heeft dezelfde velden
     * maar geen deelvragen van zichzelf. Dieper is een boom, en wie die nodig
     * heeft, heeft eerder een nieuw werkpakket nodig dan een derde laag.
     */
    onderzoeksvragen: z
      .array(
        z.union([
          z.string(),
          z
            .object({
              ...vraagVelden,
              deelvragen: z
                .array(z.union([z.string(), z.object(vraagVelden).strict()]))
                .default([]),
              // Alleen op een hoofdvraag. Een deelvraag is een stap in het
              // beantwoorden van zijn vraag; het werk hangt aan de vraag.
              stories: z.array(z.object(storyVelden).strict()).default([]),
            })
            .strict(),
        ]),
      )
      .default([]),
    samenhangIds: z.array(slug()).default([]),
    // Werkpakketten that have to be delivered before this one can start.
    // Distinct from samenhangIds, which is a mutual "these belong together"
    // and carries no order: this one is a direction in time, so it is written
    // one way round and read both ways (the werkpakket page derives the
    // reverse list itself). assertReferencesResolve() checks that every id
    // exists, that nothing depends on itself, and that the graph has no cycle.
    afhankelijkVan: z.array(slug()).default([]),
    // Two axes that genuinely diverge, so two fields rather than one.
    // A question can be answered without anything being built (onderzoek
    // 'beantwoord', bouw 'niet'), and a thing can be built while the question
    // behind it stays open (the reverse). Collapsing them into one status
    // would force a choice the roadmap cannot make.
    //
    // Both default to the least-claim value, so an existing werkpakket keeps
    // meaning what it meant: nothing asserted is not the same as "nothing
    // done", and the pages render an unset field as "niet bepaald".
    onderzoek: z.enum(ONDERZOEK_IDS).or(z.literal('')).default(''),
    bouw: z.enum(BOUW_IDS).or(z.literal('')).default(''),
    /*
     * Of het werkpakket belegd is: of iemand het heeft opgepakt.
     *
     * Een derde as naast `onderzoek` en `bouw`, en met opzet geen vierde
     * voortgangsveld — dit zegt of er iemand op zit, niet hoe ver het is.
     * Zie BELEGGING_STANDEN in lib/roadmap.ts voor de standen en het
     * verschil tussen '' (niets gezegd) en 'vrij' (op te pakken).
     *
     * Eén object in plaats van losse velden: het is één feit, in één
     * bewerking geschreven, en alleen zo is te controleren dat een datum
     * zonder stand niets betekent.
     *
     * Het hele object mag ontbreken; dat is de minste-bewering-waarde en
     * waar alle negenenveertig werkpakketten beginnen. `sinds` is een
     * string en geen z.date(), om dezelfde reden als de datum van een RFC:
     * geen tijdzoneverschuiving in de build.
     *
     * Er staat met opzet geen naam in, en geen lijst met issues of pull
     * requests. De roadmap is publiek en vanaf de homepage gelinkt, en wie
     * eraan werkt blijkt al uit de pull requests: die dragen een
     * `Werkpakket: <slug>`-regel, en de werkpakketpagina zoekt daarop. Die
     * index onderhoudt zichzelf; een lijst hier zou een tweede waarheid zijn
     * die veroudert zodra iemand vergeet hem bij te werken.
     */
    belegging: z
      .object({
        stand: z.enum(BELEGGING_IDS).or(z.literal('')).default(''),
        sinds: z
          .string()
          .regex(/^\d{4}-\d{2}-\d{2}$/, "moet 'JJJJ-MM-DD' zijn")
          .optional(),
      })
      .default({ stand: '' })
      .superRefine((b, ctx) => {
        // Zonder datum is niet te zien of dit vorige week of vorig jaar is
        // opgepakt, en dat is precies wat een verjaarde claim zichtbaar maakt.
        if ((b.stand === 'opgepakt' || b.stand === 'klaar') && !b.sinds) {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            path: ['sinds'],
            message: `belegging.stand '${b.stand}' vereist een \`sinds\`.`,
          });
        }
        // Gegevens bij een stand die ze nergens rendert zouden stil zijn.
        if (b.stand !== 'opgepakt' && b.stand !== 'klaar' && b.sinds) {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            path: ['stand'],
            message:
              '`sinds` ingevuld terwijl `stand` niet ' +
              "'opgepakt' of 'klaar' is; die datum rendert dan nergens.",
          });
        }
      }),
    // RFC numbers whose design work belongs to this werkpakket, e.g. [13, 21].
    // The RFC keeps its own `implementation` field; this is a pointer, not a
    // copy of it. assertRfcReferences() checks each number exists.
    rfcs: z.array(z.number().int().positive()).default([]),
  }),
});

export const collections = { docs, rfcs, werkpakketten };
