/**
 * How a tile tells its outcome.
 *
 * Without this every tile says the same thing: "Je voldoet aan de voorwaarden",
 * the amount, and the field name underneath. That is a label, not an answer —
 * the citizen has to work out for themselves what it means for them. The POC
 * wrote a sentence around the number instead ("Je huurtoeslag is waarschijnlijk
 * € 302,96 per jaar", "Voor de verkiezingen van 29 oktober 2025 heb je
 * STEMRECHT"), and that reads as something addressed to this person.
 *
 * The wording is content, not code: it lives in `demo-config.yaml` under
 * `outcome_phrasing`, keyed by service and law path. A law that has no entry
 * keeps the general rendering, so this can grow law by law.
 */

/** One law's entry in `outcome_phrasing` in `demo-config.yaml`. */
export interface OutcomePhrasing {
  lead?: string;
  lead_no_date?: string | null;
  yes?: string;
  no?: string;
  none?: string;
  unit?: string | null;
  date_input?: string | null;
}

/** What `phraseOutcome` needs to know about the outcome. */
export interface OutcomeFacts {
  /** did the law's conditions hold */
  met?: boolean;
  /** the formatted amount, already in euros */
  value?: string | null;
  /** the amount is unknown (RFC-036): facts are missing */
  unknown?: boolean;
  /** is the primary output a yes/no */
  isYesNo?: boolean;
  /** formatted date for `{date}` in the lead */
  date?: string | null;
}

/** The sentence a tile shows: a lead, the headline (an amount or a verdict) and its unit. */
export interface PhrasedOutcome {
  lead: string;
  headline: string;
  unit: string | null;
}

/** The phrasing for one law, or null when it has none. */
export function phrasingFor(
  config: { outcome_phrasing?: Record<string, OutcomePhrasing | undefined> | null } | null | undefined,
  service: string,
  lawPath: string,
): OutcomePhrasing | null {
  return config?.outcome_phrasing?.[`${service}/${lawPath}`] ?? null;
}

/**
 * The sentence a tile shows, or null to fall back to the general rendering.
 *
 * @param phrasing the law's entry in `outcome_phrasing`
 */
export function phraseOutcome(
  phrasing: OutcomePhrasing | null | undefined,
  { met, value, unknown = false, isYesNo = false, date = null }: OutcomeFacts = {},
): PhrasedOutcome | null {
  if (!phrasing) return null;
  // An unknown amount is not "nothing": the general rendering names what is
  // missing. Read as `none`, Claudia's precario tile said she probably owed no
  // precariobelasting while only the terrace area was not known yet.
  if (met && unknown) return null;

  // A yes/no law states the verdict itself; there is no amount to show.
  if (isYesNo || phrasing.yes || phrasing.no) {
    const headline = met ? phrasing.yes : phrasing.no;
    if (!headline || !phrasing.lead) return null;
    return { lead: fillDate(phrasing.lead, date, phrasing.lead_no_date), headline, unit: null };
  }

  // Conditions not met, or nothing granted: one plain sentence, no number.
  if (!met || value === null || value === undefined) {
    return phrasing.none ? { lead: '', headline: phrasing.none, unit: null } : null;
  }

  if (!phrasing.lead) return null;
  return { lead: fillDate(phrasing.lead, date, phrasing.lead_no_date), headline: value, unit: phrasing.unit ?? null };
}

/**
 * Put the date the outcome is about into the lead.
 *
 * Without a date the whole clause around the placeholder goes, not just the
 * placeholder: "Voor de verkiezingen van {date} heb je" has to become "Voor
 * de verkiezingen heb je", never "Voor de verkiezingen van heb je". A
 * `lead_no_date` in the config says what to fall back to, because only the
 * author of the sentence knows which words belonged to the date. Without one
 * the placeholder is dropped and the whitespace collapsed, which is right for
 * a lead that ends on the date ("Per {date} heb je" → "Per heb je" is still
 * wrong, so such a lead should carry `lead_no_date`).
 */
function fillDate(lead: string, date: string | null, fallback: string | null = null): string {
  if (!lead.includes('{date}')) return lead;
  if (!date) return (fallback ?? lead.replace('{date}', '')).replace(/\s{2,}/g, ' ').trim();
  return lead.replace('{date}', date).replace(/\s{2,}/g, ' ').trim();
}

/** Which input carries the date a yes/no law is about, if any. */
export function dateInputFor(phrasing: OutcomePhrasing | null | undefined): string | null {
  return phrasing?.date_input ?? null;
}
