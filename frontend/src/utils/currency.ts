/**
 * Amounts are stored as integer eurocents but displayed/entered as euros.
 * These two pure helpers convert between the two; shared by EditSheet (law
 * definition values) and ScenarioParameterInput (scenario amount inputs).
 */

/** Eurocents (int) -> euros (2-decimal number). Empty/nullish passes through. */
export function centsToEuros(cents: unknown): number | '' {
  if (cents === '' || cents == null) return '';
  return +(Number(cents) / 100).toFixed(2);
}

/** Euros (number) -> eurocents (rounded int). Empty/nullish passes through. */
export function eurosToCents(euros: unknown): number | '' {
  if (euros === '' || euros == null) return '';
  return Math.round(Number(euros) * 100);
}
