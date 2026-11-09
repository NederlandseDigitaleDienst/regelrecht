/**
 * Telt dit select-event als een échte wissel van persona? De
 * persona-switcher stuurt ook een select-event bij het synchroniseren van de
 * dropdown, dus zonder deze controle zou elke render de keuzes wissen.
 */
export function isPersonaWissel(huidig, nieuw) {
  return !!nieuw && huidig?.bsn !== nieuw.bsn;
}

/**
 * De keuzes waarmee je bij een persoon binnenkomt: wat je eerder bij deze
 * persoon aanzette, en verder diens eigen uitgangspunt.
 */
export function keuzesBijBinnenkomst(standaard, bewaard) {
  return { ...standaard, ...(bewaard ?? {}) };
}
