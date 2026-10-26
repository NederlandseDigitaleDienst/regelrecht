/**
 * What a law article produces (`execution.produces`), in either form the
 * schema allows: one object, the form most laws use, or a list of produced
 * items, such as an application and the decision on it (schema v0.8.1,
 * RFC-047). A list item says what it is with `type`: a submission kind
 * (`AANVRAAG`) or a legal character (`BESCHIKKING`). The object form says it
 * with `submission.kind` and `legal_character`. Readers go through these
 * helpers so neither form is missed.
 */

/** The submission kinds a list item's `type` can name; any other is a legal character. */
export const SUBMISSION_KINDS: readonly string[] = ['AANVRAAG', 'BEZWAAR', 'BEROEP'];

/**
 * One produced item, in either form. The list form names what it is with
 * `type`; the object form with `legal_character` and `submission.kind`. The
 * other fields (`decision_type`, `procedure_id`, ...) are carried as written.
 * Read from YAML, so nothing here is validated beyond what the helpers check.
 */
export interface ProducedItem {
  type?: string;
  legal_character?: string;
  submission?: { kind?: string };
  decision_type?: string;
  procedure_id?: string;
  [field: string]: unknown;
}

/** `execution.produces` as written: one object, a list, or absent. */
export type Produces = ProducedItem | ProducedItem[] | null | undefined;

/** The decision in the object form's terms, as `decisionOf` returns it. */
export interface Decision {
  legal_character: string;
  decision_type: string | undefined;
  procedure_id: string | undefined;
}

/** Every produced item, in the order written: one for the object form. */
export function producedItems(produces: Produces): ProducedItem[] {
  if (produces == null) return [];
  return Array.isArray(produces) ? produces : [produces];
}

/** Whether `item` is a list item (it has a `type`), not the object form. */
export function isListItem(item: ProducedItem | null | undefined): item is ProducedItem & { type: string } {
  return item != null && typeof item.type === 'string';
}

/** The legal character of `item`, or `null` when it is no decision. */
export function legalCharacterOf(item: ProducedItem | null | undefined): string | null {
  if (item == null) return null;
  if (isListItem(item)) return SUBMISSION_KINDS.includes(item.type) ? null : item.type;
  return item.legal_character ?? null;
}

/** The kind of submission `item` establishes, or `null`. */
export function submissionKindOf(item: ProducedItem | null | undefined): string | null {
  if (item == null) return null;
  if (isListItem(item)) return SUBMISSION_KINDS.includes(item.type) ? item.type : null;
  return item.submission?.kind ?? null;
}

/**
 * The decision the article produces, as written: in a list the item with a
 * decision `type`; the object form is the decision itself. `null` when there
 * is none. Read its legal character with `legalCharacterOf`.
 */
export function producedDecision(produces: Produces): ProducedItem | null {
  if (produces == null) return null;
  if (!Array.isArray(produces)) return produces;
  return produces.find((p) => legalCharacterOf(p) != null) ?? null;
}

/** The item that establishes a submission, or `null`. */
export function producedSubmission(produces: Produces): ProducedItem | null {
  return producedItems(produces).find((p) => submissionKindOf(p) != null) ?? null;
}

/**
 * The decision in the object form's terms, whatever form it was written in:
 * `{ legal_character, decision_type, procedure_id }`, or `null` when the
 * article takes no decision. For readers that show or test the decision.
 */
export function decisionOf(produces: Produces): Decision | null {
  const decision = producedDecision(produces);
  const legalCharacter = legalCharacterOf(decision);
  if (decision == null || legalCharacter == null) return null;
  return {
    legal_character: legalCharacter,
    decision_type: decision.decision_type,
    procedure_id: decision.procedure_id,
  };
}

/**
 * `produces` with one field of its decision set to `data`, or cleared when
 * `data` is `null`, as the editor's dropdowns patch it; `undefined` when
 * nothing is left, so no bare `produces: {}` reaches the YAML. Does not
 * mutate `produces`.
 *
 * In a list the field belongs to the decision item, and `legal_character` is
 * that item's `type`. Setting a legal character on a list without a decision
 * adds one, and clearing it removes the item, which is nothing without a
 * type. Any other field of a list without a decision is left alone: there is
 * no decision to hold it yet.
 */
export function withDecisionField(
  produces: Produces,
  key: string,
  data: unknown,
): ProducedItem | ProducedItem[] | undefined {
  if (!Array.isArray(produces)) {
    const next: ProducedItem = { ...(produces ?? {}) };
    if (data == null) delete next[key];
    else next[key] = data;
    return Object.keys(next).length > 0 ? next : undefined;
  }
  const field = key === 'legal_character' ? 'type' : key;
  const list = produces.map((item) => ({ ...item }));
  let target = producedDecision(list);
  if (!target) {
    if (data == null || field !== 'type') return produces;
    target = {};
    list.push(target);
  }
  if (data == null) delete target[field];
  else target[field] = data;
  const kept = target.type == null ? list.filter((item) => item !== target) : list;
  return kept.length > 0 ? kept : undefined;
}
