import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount } from '@vue/test-utils';
import TrajectIntegrityPane from './TrajectIntegrityPane.vue';
import { groupByLaw, impactSummary } from '../composables/useTrajectIntegrity';
import type { FindingGroup, IntegrityFinding } from '../composables/useTrajectIntegrity';

// De pane haalt het rapport via useTrajectIntegrity -> apiFetchJson op; die
// ene netwerkpoot sturen we hier (zelfde mock-vorm als TasksCategoriesPane).
const apiFetchJson = vi.fn();
vi.mock('../lib/apiFetch', () => ({
  apiFetchJson: (...a: unknown[]) => apiFetchJson(...a),
}));

// Fictieve wetten en mapnamen: dit is een publieke repo, dus geen echte
// traject- of repo-namen in fixtures.
const ERROR_FINDING = {
  severity: 'error',
  kind: 'directory_name_mismatch',
  path: 'waterschaps_verordening/hoogland/keur_alpha',
  law_id: 'keur_alpha_hoogland',
  message: "Het regelwerk in de map 'keur_alpha' heeft '$id: keur_alpha_hoogland'.",
  remedy: "Hernoem de map naar 'keur_alpha_hoogland'.",
};
const WARNING_FINDING = {
  severity: 'warning',
  kind: 'scenario_directory_without_target',
  path: 'wet/wet_alpha/scenarios',
  law_id: 'wet_alpha',
  message: "De scenario's evalueren 'wet_alpha' niet.",
  remedy: 'Voeg een evaluatiestap toe.',
};

const CLEAN_REPORT = {
  traject_ref: 'voorbeeldtraject-abcd1234',
  source_id: 'traject-own',
  checked_laws: 12,
  checked_scenarios: 3,
  findings: [],
};
const DIRTY_REPORT = {
  ...CLEAN_REPORT,
  findings: [ERROR_FINDING, WARNING_FINDING],
};

// Narrow a `find()` result; throws (failing the test) when nothing matched.
function found<T>(value: T | undefined): T {
  if (value === undefined) throw new Error('expected a match');
  return value;
}

// Index into a group list and fail loudly instead of reading through undefined.
function groupAt(groups: FindingGroup[], i: number): FindingGroup {
  const group = groups[i];
  if (!group) throw new Error(`no group at index ${i}`);
  return group;
}

async function mountPane() {
  const wrapper = mount(TrajectIntegrityPane, {
    props: { trajectRef: 'voorbeeldtraject-abcd1234' },
    attachTo: document.body,
  });
  // Flush de load() die onMounted aftrapt.
  await wrapper.vm.$nextTick();
  await Promise.resolve();
  await Promise.resolve();
  await wrapper.vm.$nextTick();
  return wrapper;
}

describe('groupByLaw', () => {
  it('groepeert per wet-map en zet de zwaarst getroffen wet bovenaan', () => {
    const groups = groupByLaw({ findings: [WARNING_FINDING, ERROR_FINDING] });
    expect(groups.map((g) => g.title)).toEqual(['keur_alpha', 'wet_alpha']);
    expect(groupAt(groups, 0).findings).toEqual([ERROR_FINDING]);
    expect(groupAt(groups, 0).counts).toBe('1 fout');
    expect(groupAt(groups, 1).counts).toBe('1 waarschuwing');
  });

  it('herleidt de wet-map uit versiebestanden en scenariopaden', () => {
    const inFile = { ...ERROR_FINDING, path: 'wet/wet_alpha/2024-01-01.yaml' };
    const inScenario = { ...WARNING_FINDING, path: 'wet/wet_alpha/scenarios/basis.feature' };
    const groups = groupByLaw({ findings: [inFile, inScenario] });
    expect(groups).toHaveLength(1);
    expect(groupAt(groups, 0).title).toBe('wet_alpha');
    // Binnen de wet: fouten boven waarschuwingen.
    expect(groupAt(groups, 0).findings).toEqual([inFile, inScenario]);
    expect(groupAt(groups, 0).counts).toBe('1 fout, 1 waarschuwing');
  });

  it('sorteert op impact: meer fouten eerst, dan waarschuwingen, dan alfabet', () => {
    const twoErrors = [
      { ...ERROR_FINDING, path: 'wet/wet_zwaar/a.yaml' },
      { ...ERROR_FINDING, path: 'wet/wet_zwaar/b.yaml' },
    ];
    const groups = groupByLaw({
      findings: [WARNING_FINDING, ERROR_FINDING, ...twoErrors],
    });
    expect(groups.map((g) => g.title)).toEqual(['wet_zwaar', 'keur_alpha', 'wet_alpha']);
  });

  it('geeft een lege lijst voor een leeg of ontbrekend rapport', () => {
    expect(groupByLaw(null)).toEqual([]);
    expect(groupByLaw(CLEAN_REPORT)).toEqual([]);
  });

  it('zet bevindingen zonder pad samen in een traject-brede groep', () => {
    // The backend omits `path` rather than sending null; null is the defensive case.
    const pathless = { ...ERROR_FINDING, path: null } as unknown as IntegrityFinding;
    const groups = groupByLaw({ findings: [pathless] });
    expect(groups.map((g) => g.title)).toEqual(['Traject-breed']);
  });

  it('laat een onbekende severity niet verdwijnen maar achteraan in de groep belanden', () => {
    const future = { ...ERROR_FINDING, severity: 'toekomstig' };
    const groups = groupByLaw({ findings: [future, ERROR_FINDING] });
    expect(groupAt(groups, 0).findings).toEqual([ERROR_FINDING, future]);
    expect(groupAt(groups, 0).counts).toBe('1 fout, 1 overig');
  });
});

describe('impactSummary', () => {
  it('telt het totaal en over hoeveel regelwerken het verdeeld is', () => {
    expect(impactSummary({ findings: [ERROR_FINDING, WARNING_FINDING] })).toBe(
      'In totaal 1 fout, 1 waarschuwing, verdeeld over 2 regelwerken.',
    );
  });

  it('is leeg zonder bevindingen', () => {
    expect(impactSummary(CLEAN_REPORT)).toBeNull();
  });
});

describe('TrajectIntegrityPane', () => {
  beforeEach(() => {
    vi.resetModules();
    apiFetchJson.mockReset();
    document.body.innerHTML = '';
  });

  it('vraagt het rapport op voor het traject uit de props', async () => {
    apiFetchJson.mockResolvedValue(CLEAN_REPORT);
    await mountPane();
    expect(apiFetchJson).toHaveBeenCalledTimes(1);
    expect(apiFetchJson.mock.calls[0]?.[0]).toBe(
      '/api/trajects/voorbeeldtraject-abcd1234/integrity',
    );
  });

  it('rendert de bevindingen gegroepeerd per wet, zwaarst getroffen eerst', async () => {
    apiFetchJson.mockResolvedValue(DIRTY_REPORT);
    const wrapper = await mountPane();
    const html = wrapper.html();

    // Eén kop per wet, met de tellers erbij; de wet met de fout bovenaan.
    expect(html).toContain('keur_alpha (1 fout)');
    expect(html).toContain('wet_alpha (1 waarschuwing)');
    expect(html.indexOf('keur_alpha (1 fout)')).toBeLessThan(
      html.indexOf('wet_alpha (1 waarschuwing)'),
    );

    // De impactregel geeft het rapport zijn maat.
    expect(html).toContain('In totaal 1 fout, 1 waarschuwing, verdeeld over 2 regelwerken.');

    // Elke bevinding draagt omschrijving + remedie + het pad waar hij zit,
    // en een eigen severity-icoon (fouten en waarschuwingen mengen per wet).
    const cells = wrapper.findAll('nldd-text-cell');
    const error = found(cells.find((c) => c.attributes('text') === ERROR_FINDING.message));
    expect(error).toBeTruthy();
    expect(error.attributes('supporting-text')).toBe(ERROR_FINDING.remedy);
    expect(error.attributes('overline')).toBe(ERROR_FINDING.path);

    const warning = found(cells.find((c) => c.attributes('text') === WARNING_FINDING.message));
    expect(warning.attributes('supporting-text')).toBe(WARNING_FINDING.remedy);

    const icons = wrapper.findAll('nldd-icon').map((i) => i.attributes('icon'));
    expect(icons).toContain('error');
    expect(icons).toContain('warning');

    // Geen "alles in orde"-melding als er wél iets is.
    expect(html).not.toContain('Geen problemen gevonden');
  });

  it('toont een bevestigende lege staat als er niets mis is', async () => {
    apiFetchJson.mockResolvedValue(CLEAN_REPORT);
    const wrapper = await mountPane();

    const empty = found(
      wrapper.findAll('nldd-inline-dialog').find((d) => d.attributes('text') === 'Geen problemen gevonden'),
    );
    expect(empty).toBeTruthy();
    expect(empty.attributes('variant')).toBe('success');
    // De omvang van de controle staat erbij, anders leest "geen problemen"
    // als "er is niets gecontroleerd".
    expect(empty.attributes('supporting-text')).toContain('12 versies');
    expect(empty.attributes('supporting-text')).toContain("3 scenario's");
  });

  it('toont de foutmelding van de backend als de controle niet kon draaien', async () => {
    apiFetchJson.mockRejectedValue(new Error('GitHub is nu niet bereikbaar.'));
    const wrapper = await mountPane();

    const alert = found(
      wrapper.findAll('nldd-inline-dialog').find((d) => d.attributes('variant') === 'alert'),
    );
    expect(alert).toBeTruthy();
    expect(alert.attributes('text')).toBe('Structuurcontrole niet gelukt');
    expect(alert.attributes('supporting-text')).toBe('GitHub is nu niet bereikbaar.');
    // Geen rapport, dus ook geen lege staat die suggereert dat alles klopt.
    expect(wrapper.html()).not.toContain('Geen problemen gevonden');
  });

  it('haalt het rapport opnieuw op via de verversknop', async () => {
    apiFetchJson.mockResolvedValue(CLEAN_REPORT);
    const wrapper = await mountPane();
    expect(apiFetchJson).toHaveBeenCalledTimes(1);

    const refresh = found(
      wrapper.findAll('nldd-button').find((b) => b.attributes('text') === 'Opnieuw controleren'),
    );
    expect(refresh).toBeTruthy();
    await refresh.trigger('click');
    await wrapper.vm.$nextTick();
    expect(apiFetchJson).toHaveBeenCalledTimes(2);
  });
});
