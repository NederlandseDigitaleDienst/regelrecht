<script setup>
// De scenario's die je in de app kunt naspelen: per scenario de stappen in
// volgorde (welk tabblad, welke knop, welke waarde, welke speeldatum) en wat
// je aan het eind ziet. De knop maakt de kroniek leeg en zet de speeldatum
// op de eerste dag; daarna ga je naar het tabblad Ouder.
import { api } from '../api.js';
import { spel, stap } from '../spel.js';

const emit = defineEmits(['start']);

const scenarios = [
  {
    naam: 'Aanvraag wordt verleend',
    stappen: [
      'Ouder, speeldatum 14 oktober 2026: klik op Aanvraag indienen. De voorbeeldwaarden vragen verlof van 1 tot en met 5 februari 2027.',
      'School: laat beide bevindingen aan staan en klik op Besluit nemen.',
    ],
    eind: 'Onder Besluiten staat Verlof verleend. Bij Ouder staat de aanvraag op Verleend.',
  },
  {
    naam: 'Afwijzing, bezwaar en heroverweging: gegrond',
    stappen: [
      'Ouder, 14 oktober 2026: klik op Aanvraag indienen.',
      'School: zet "specifieke aard beroep ouder" uit. Het hoofd vindt het beroep niet bijzonder genoeg. Klik op Besluit nemen. Het verlof wordt geweigerd.',
      'Ouder, speeldatum 1 november 2026: klik bij het besluit op Bezwaar maken. De gronden noemen een verklaring van de werkgever. Klik op Bezwaar indienen.',
      'School, speeldatum 15 november 2026: klik op Ontvankelijkheid toetsen. Het bezwaar is ontvankelijk.',
      'Kies bij "specifieke aard beroep ouder" Ja: het hoofd beoordeelt het beroep nu anders. Klik op Beslissen op bezwaar.',
    ],
    eind: 'Gegrond: het besluit is herroepen. Er is een nieuw besluit, en dat verleent het verlof. In de Kroniek staan vijf chronolexogrammen.',
  },
  {
    naam: 'Afwijzing, bezwaar en heroverweging: ongegrond',
    stappen: [
      'Ouder, 14 oktober 2026: klik op Aanvraag indienen. School: klik op Besluit nemen. Het verlof wordt verleend.',
      'Ouder: vul als eerste verlofdag 10 mei 2027 en als laatste verlofdag 14 mei 2027 in. Klik op Aanvraag indienen.',
      'School: klik op Besluit nemen. Het verlof wordt geweigerd: de jongere kreeg dit schooljaar al verlof.',
      'Ouder, speeldatum 1 november 2026: klik bij het tweede besluit op Bezwaar maken en dan op Bezwaar indienen.',
      'School, speeldatum 15 november 2026: klik op Ontvankelijkheid toetsen. Laat alle bevindingen staan en klik op Beslissen op bezwaar.',
    ],
    eind: 'Ongegrond: de heroverweging geeft dezelfde uitkomst. Het besluit blijft, er komt geen nieuw besluit.',
  },
  {
    naam: 'Bezwaar te laat: niet-ontvankelijk',
    stappen: [
      'Ouder, 14 oktober 2026: klik op Aanvraag indienen. School: zet "specifieke aard beroep ouder" uit en klik op Besluit nemen.',
      'Ouder, speeldatum 1 december 2026: de bezwaartermijn is voorbij. Klik op Bezwaar maken en dan op Bezwaar indienen.',
      'School: klik op Ontvankelijkheid toetsen.',
    ],
    eind: 'Niet-ontvankelijk: het bezwaar kwam te laat. Er is geen heroverweging, dus geen vragen aan het hoofd. Na Beslissen op bezwaar blijft het besluit staan.',
  },
  {
    naam: 'Tweede bezwaar tegen een vervangen besluit',
    stappen: [
      'Ouder, 14 oktober 2026: klik op Aanvraag indienen. School: klik op Besluit nemen. Het verlof wordt verleend.',
      'Ouder, speeldatum 1 november 2026: maak bezwaar tegen het besluit. Speeldatum 2 november: maak nog een bezwaar tegen hetzelfde besluit.',
      'School, speeldatum 15 november 2026: toets het eerste bezwaar. Kies bij "specifieke aard beroep ouder" Nee en beslis. Gegrond: een weigering komt in de plaats van het besluit.',
      'Speeldatum 16 november 2026: toets het tweede bezwaar. Kies weer Nee en beslis.',
    ],
    eind: 'De cel weigert deze stap: een besluit wordt maar één keer vervangen. De melding noemt het besluit dat het al vervangt; onder Details staat wat de cel zelf meldt. Er komt niets bij in de kroniek.',
  },
  {
    naam: 'Een herroepen verlof telt niet meer mee',
    stappen: [
      'Ouder, 14 oktober 2026: klik op Aanvraag indienen (verlof A, 1 tot en met 5 februari 2027). School: klik op Besluit nemen. A wordt verleend.',
      'Ouder: vul als eerste verlofdag 10 mei 2027 en als laatste verlofdag 14 mei 2027 in en klik op Aanvraag indienen (verlof B). School: klik op Besluit nemen. B wordt geweigerd: de school antwoordt uit haar kroniek dat de jongere dit schooljaar al verlof kreeg, namelijk A.',
      'Ouder, speeldatum 1 november 2026: maak bezwaar tegen A.',
      'School, speeldatum 15 november 2026: toets dat bezwaar. Kies bij "specifieke aard beroep ouder" Nee en beslis. Gegrond: A is herroepen, een weigering komt in de plaats.',
      'Ouder, speeldatum 20 november 2026: maak bezwaar tegen B.',
      'School, speeldatum 1 december 2026: toets dat bezwaar, laat de bevindingen staan en beslis.',
    ],
    eind: 'Gegrond: B is herroepen, en het nieuwe besluit verleent het verlof. Bij het waarom staat "verlof al verleend in schooljaar: nee", ingevuld door de school. Onder "Hoe de school dit beantwoordde" staat A bij de herroepen besluiten, en het spoor laat zien dat het filter twee besluiten onder artikel 13a oversloeg. Dat zijn A en de weigering die ervoor in de plaats kwam.',
  },
];

async function begin() {
  const ok = await stap(() => api.reset());
  if (ok) {
    spel.datum = '2026-10-14';
    emit('start');
  }
}
</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" size="3" text="Scenario's">
      <nldd-text slot="supporting-text">
        Zo speel je het proces na. Elk scenario begint met een lege kroniek op 14 oktober 2026.
        De speeldatum staat bovenaan en loopt alleen vooruit.
      </nldd-text>
    </nldd-title>
    <nldd-container gap="16">
      <nldd-card v-for="(s, i) in scenarios" :key="s.naam">
        <nldd-container padding="16" gap="12">
          <nldd-title size="5" :overline="`Scenario ${i + 1}`" :text="s.naam"></nldd-title>
          <nldd-rich-text>
            <ol>
              <li v-for="(t, j) in s.stappen" :key="j">{{ t }}</li>
            </ol>
            <p><strong>Je ziet:</strong> {{ s.eind }}</p>
          </nldd-rich-text>
          <nldd-button appearance="secondary" text="Begin met een lege kroniek" @click="begin"></nldd-button>
        </nldd-container>
      </nldd-card>
    </nldd-container>
  </nldd-simple-section>
</template>
