<template>
  <nldd-list appearance="box-tinted">
    <nldd-list-item v-for="stap in stappen" :key="stap.label">
      <nldd-icon-cell :icon="stap.icon" :color="stap.color ?? 'content'"></nldd-icon-cell>
      <nldd-text-cell :text="stap.label" :supporting-text="stap.uitleg"></nldd-text-cell>
      <nldd-text-cell width="fit-content" horizontal-alignment="right">
        <nldd-tag v-if="stap.tag" size="sm" :color="stap.tagColor ?? 'neutral'" :text="stap.tag"></nldd-tag>
        <strong v-else>{{ stap.waarde }}</strong>
      </nldd-text-cell>
    </nldd-list-item>
  </nldd-list>
</template>

<script setup>
import { computed } from 'vue';
import {
  datumLabel,
  addMonths,
  categorieLabel,
  categorieColor,
  CODES_BESTUUR_BEOORDEELT,
  tabbladVoor,
  TABBLAD_LABELS,
} from '../../lib/nieuwkomerFacts.js';

const props = defineProps({
  persona: { type: Object, required: true },
  /** Eerste uitkomst (huidig recht) waarin de leerling in het bestand zit, of de eerste tellende. */
  uitkomst: { type: Object, default: null },
});

const stappen = computed(() => {
  const p = props.persona;
  const u = props.uitkomst;
  const sector = p.sector === 'vo' ? 'vo' : 'po';
  const out = [];

  if (sector === 'po') {
    const code = p.verblijfstitel_code;
    const codeTekst = !p.heeft_bsn
      ? 'Geen BSN: ingeschreven op onderwijsnummer, het bestuur beoordeelt (tabblad 3).'
      : code == null
        ? 'Verblijfstitelcode onbekend in ROD.'
        : CODES_BESTUUR_BEOORDEELT.includes(Number(code))
          ? `Code ${code} staat niet in lid 10; het bevoegd gezag beslist (lid 11, tabblad 2).`
          : `Code ${code} uit ROD/BRP bepaalt de categorie (art. 34 lid 10, tabblad 1).`;
    out.push({
      icon: 'tag',
      label: 'Categorie',
      uitleg: codeTekst + (p.oordeel_bevoegd_gezag ? ` Oordeel bestuur: ${categorieLabel(p.oordeel_bevoegd_gezag)}.` : ''),
      tag: categorieLabel(u?.categorie ?? '—'),
      tagColor: categorieColor(u?.categorie),
    });
    out.push({
      icon: 'table-cells',
      label: 'Bestand Nieuwkomers',
      uitleg: 'Waar de leerling in het DUO-bestand verschijnt.',
      waarde: TABBLAD_LABELS[tabbladVoor(p, u?.categorie)],
    });
    const vierde = p.geboortedatum ? addMonths(p.geboortedatum, 48) : null;
    const voorVierde = vierde && p.datum_vestiging_nederland && p.datum_vestiging_nederland < vierde;
    out.push({
      icon: 'calendar-event',
      label: 'Vierde verjaardag',
      uitleg: voorVierde
        ? `Gevestigd op ${datumLabel(p.datum_vestiging_nederland)}, vier jaar op ${datumLabel(vierde)}: de tijd ertussen gaat van het recht af (art. 34 lid 4), afgerond op hele kwartalen naar beneden.`
        : `Gevestigd op ${datumLabel(p.datum_vestiging_nederland)}, al vier jaar of ouder: geen aftrek (art. 34 lid 4).`,
      waarde: u?.aftrek_kwartalen != null
        ? `${u.aftrek_maanden ?? '?'} maanden → ${u.aftrek_kwartalen} kwartaal${u.aftrek_kwartalen === 1 ? '' : 'en'} aftrek`
        : '—',
      color: voorVierde ? 'warning' : 'content',
    });
    out.push({
      icon: 'timer',
      label: 'Bekostigbare kwartalen',
      uitleg: u?.categorie === 'ASIELZOEKER' || u?.categorie_effectief === 'ASIELZOEKER'
        ? 'Asielzoeker: acht kwartalen (twaalf maanden art. 34, twaalf maanden art. 35), minus de aftrek.'
        : 'Overige vreemdeling: vier kwartalen (art. 34), minus de aftrek.',
      waarde: u?.bekostigbare_kwartalen != null ? `${u.bekostigbare_kwartalen} kwartalen` : '—',
    });
    out.push({
      icon: 'flag',
      label: 'Eerste inschrijving',
      uitleg: `Het recht start op de eerste dag van inschrijving op een school in Nederland (art. 34 lid 3): ${datumLabel(p.eerste_inschrijfdatum)}. De eerste peildatum daarna telt als eerste kwartaal.`,
      waarde: datumLabel(p.eerste_inschrijfdatum),
    });
  } else {
    out.push({
      icon: 'tag',
      label: 'Nieuwkomer (vo)',
      uitleg: 'Vreemdeling, werkelijk schoolgaand, in Nederland, en korter dan twee jaar geleden voor het eerst ingeschreven (Uitvoeringsbesluit WVO 2020 art. 1.1). Geen asiel/overig-onderscheid.',
      tag: u?.is_nieuwkomer === false ? 'Geen nieuwkomer' : u?.is_nieuwkomer ? 'Nieuwkomer' : '—',
      tagColor: u?.is_nieuwkomer ? 'lintblauw' : 'neutral',
    });
    out.push({
      icon: 'flag',
      label: 'Eerste inschrijving',
      uitleg: `${datumLabel(p.eerste_inschrijfdatum)}; het tweejaarsvenster loopt tot ${datumLabel(addMonths(p.eerste_inschrijfdatum, 24))}.`,
      waarde: datumLabel(p.eerste_inschrijfdatum),
    });
    out.push({
      icon: 'calendar-event',
      label: 'Categorie naar teldatum',
      uitleg: 'Eerste categorie: nog niet ingeschreven op 1 oktober van het vorige jaar (niet in de reguliere telling, hoog tarief). Tweede categorie: wel, en binnen twee jaar na de eerste inschrijving (laag tarief).',
      tag: u?.categorie ? categorieLabel(u.categorie) : '—',
      tagColor: categorieColor(u?.categorie),
    });
    out.push({
      icon: 'check-list',
      label: 'Ambtshalve',
      uitleg: 'DUO leest ROD op de zestiende na de peildatum en stelt vast zonder aanvraag (art. 3); de accountant valideert (art. 5).',
      waarde: u?.aanvraag_vereist ? 'aanvraag vereist' : 'geen aanvraag',
    });
  }
  return out;
});
</script>
