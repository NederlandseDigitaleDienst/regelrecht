// Het bouwplan van de verkenning, gedeeld door de tabs Toets en Simuleren in het kaartpaneel.
// Aannames, instelbaar: standaard 5 lagen wonen, 3,2 meter per laag.
import { reactive } from 'vue';

// soortGebouw: TAM Het Ambacht 54 (parkeerhub, stadswoning of overig). voegtWoningenToe: Woongebieden 13.2.
export const bouwplan = reactive({
  lagen: 5, meterPerLaag: 3.2, woningtype: 'gestapeld', aantalWoningen: 20, soortGebouw: 'overig', voegtWoningenToe: true,
});

export const WONINGTYPES = [
  { waarde: 'gestapeld', tekst: 'Gestapeld' },
  { waarde: 'aaneengebouwd', tekst: 'Aaneen' },
  { waarde: 'twee-aaneen', tekst: 'Twee-aaneen' },
  { waarde: 'vrijstaand', tekst: 'Vrijstaand' },
];

export const SOORTEN_GEBOUW = [
  { waarde: 'overig', tekst: 'Overig' },
  { waarde: 'stadswoning', tekst: 'Stadswoning' },
  { waarde: 'parkeerhub', tekst: 'Parkeerhub' },
];
