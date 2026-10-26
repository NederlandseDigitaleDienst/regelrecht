import { describe, it, expect, beforeAll, beforeEach, afterEach, vi, type MockInstance } from 'vitest';
import { mount, type VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import EditSheet, { type EditSheetItem } from './EditSheet.vue';

/** A source.parameters row as the tests read and write it. */
type TestParamRow = { key: string; value: string; _rowId?: number };

/**
 * The form state as the tests use it, after the watch has filled it for the
 * section under test.
 */
interface TestFormValues {
  name: string;
  type: string;
  required?: boolean;
  sourceRegulation: string;
  sourceOutput: string;
  sourceParameters: TestParamRow[];
}

/** The script-setup bindings the tests drive through wrapper.vm. */
interface EditSheetVm {
  values: TestFormValues;
  save(): void;
  allLaws: { law_id: string }[];
  availableOutputs: unknown[];
  onLawComboChange(event: { detail: { value: string } }): Promise<void>;
  onOutputComboChange(event: { detail: { value: string } }): void;
  displayName(law: { display_name: string | null; name: string | null; law_id: string }): string;
}

type SheetWrapper = Omit<VueWrapper, 'vm'> & { vm: EditSheetVm };

/** An emitted save payload, loosely typed: each test reads the fields it asserts on. */
type SavedPayload = {
  section: string;
  key?: string;
  data: { source: { parameters?: unknown }; type_spec?: unknown; [key: string]: unknown };
};

/** The first argument of the first emitted event; a missing event fails the test. */
function firstSave(events: unknown[][] | undefined): SavedPayload {
  const first = events?.[0];
  if (!first) throw new Error('no save event emitted');
  return first[0] as SavedPayload;
}

/** A fetch Response stand-in with only what apiFetchJson reads. */
function fakeResponse(ok: boolean, body: unknown): Response {
  return { ok, json: async () => body } as unknown as Response;
}

// Vite is configured with `isCustomElement: tag => tag.startsWith('nldd-')`
// (vite.config.js), so Vue's compiler emits raw HTML elements for the
// `<nldd-sheet>` tag rather than Vue components. The Vue Test Utils
// `stubs:` map doesn't apply to those - the template ref ends up pointing
// at a real HTMLElement in happy-dom, which has no `show()` / `hide()`
// methods, so EditSheet's `watch(item, ...)` throws an unhandled
// rejection on every mount.
//
// Register a minimal custom element that *does* expose show/hide so the
// watcher runs cleanly. We don't care about visual rendering - the tests
// drive the component via `wrapper.vm.values` / `wrapper.vm.save()`.
beforeAll(() => {
  if (typeof customElements !== 'undefined' && !customElements.get('nldd-sheet')) {
    class NddSheetTestStub extends HTMLElement {
      show() {}
      hide() {}
    }
    customElements.define('nldd-sheet', NddSheetTestStub);
  }
});

/**
 * EditSheet relies on `nldd-sheet`'s `show()` / `hide()` methods which only
 * exist on the real custom element. In jsdom there is no element backing
 * those methods, so we stub the ref before the watch fires by mounting
 * with a sheetEl shim.
 *
 * Each test mounts EditSheet with `attachTo` so the ref resolution
 * doesn't crash, then sets a `props.item` to drive the watch and exercise
 * each section's data flow.
 */
function mountSheet(item: EditSheetItem | null = null): SheetWrapper {
  // No stubs needed: nldd-* tags are treated as raw HTML by Vue's compiler
  // (see vite.config.js `isCustomElement`), and `nldd-sheet` is registered
  // above with no-op show/hide so the watcher in EditSheet doesn't crash.
  // The remaining nldd-* elements render as empty <nldd-*> nodes; the tests
  // poke values directly through `wrapper.vm.values` and exercise the
  // emitted save payloads, not the rendered form.
  const wrapper = mount(EditSheet, {
    attachTo: document.body,
    props: { item },
  });
  // Script-setup bindings are reachable on vm at runtime, but not in its type.
  return wrapper as unknown as SheetWrapper;
}

async function setItem(wrapper: SheetWrapper, item: EditSheetItem) {
  await wrapper.setProps({ item });
  await nextTick();
}

describe('EditSheet', () => {
  describe('definition', () => {
    it('emits save with renamed key when editing an existing definition', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'definition',
        key: 'old_name',
        rawDef: { value: 100 },
      });

      wrapper.vm.values.name = 'new_name';
      wrapper.vm.save();
      await nextTick();

      const events = wrapper.emitted('save');
      expect(events).toBeTruthy();
      expect(firstSave(events)).toMatchObject({
        section: 'definition',
        key: 'old_name',
        newKey: 'new_name',
      });
    });

    it('emits save with add-definition section for new definitions', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'add-definition',
        isNew: true,
      });

      wrapper.vm.values.name = 'foo';
      wrapper.vm.save();
      await nextTick();

      const events = wrapper.emitted('save');
      expect(firstSave(events).section).toBe('add-definition');
      expect(firstSave(events).key).toBe('foo');
    });
  });

  describe('parameter', () => {
    it('emits save with parameter data populated from item', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'parameter',
        index: 0,
        data: { name: 'bsn', type: 'string', required: true },
      });

      // Watch should hydrate values from item.data
      expect(wrapper.vm.values).toMatchObject({
        name: 'bsn',
        type: 'string',
        required: true,
      });

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(firstSave(events)).toMatchObject({
        section: 'parameter',
        index: 0,
        data: { name: 'bsn', type: 'string', required: true },
      });
    });
  });

  describe('input', () => {
    it('emits save with source.regulation and source.output', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'add-input',
        isNew: true,
      });

      // Fill name, sourceRegulation, sourceOutput by mutating the values
      // ref directly - querying inputs by index is brittle when several
      // text-field stubs render.
      wrapper.vm.values.name = 'leeftijd';
      wrapper.vm.values.type = 'number';
      wrapper.vm.values.sourceRegulation = 'wet_basisregistratie_personen';
      wrapper.vm.values.sourceOutput = 'leeftijd';

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(firstSave(events).section).toBe('add-input');
      expect(firstSave(events).data).toMatchObject({
        name: 'leeftijd',
        type: 'number',
        source: {
          regulation: 'wet_basisregistratie_personen',
          output: 'leeftijd',
        },
      });
    });

    it('emits save with source.parameters when rows are populated', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'add-input',
        isNew: true,
      });

      wrapper.vm.values.name = 'leeftijd';
      wrapper.vm.values.sourceRegulation = 'wet_basisregistratie_personen';
      wrapper.vm.values.sourceOutput = 'leeftijd';
      wrapper.vm.values.sourceParameters = [
        { key: 'bsn', value: '$bsn' },
        { key: 'peildatum', value: '2025-01-01' },
      ];

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(firstSave(events).data.source).toMatchObject({
        regulation: 'wet_basisregistratie_personen',
        output: 'leeftijd',
        parameters: { bsn: '$bsn', peildatum: '2025-01-01' },
      });
    });

    it('hydrates source.parameters from existing data on edit', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'input',
        index: 0,
        data: {
          name: 'leeftijd',
          type: 'number',
          source: {
            regulation: 'wet_basisregistratie_personen',
            output: 'leeftijd',
            parameters: { bsn: '$bsn', peildatum: '2025-01-01' },
          },
        },
      });

      // Each row also carries a stable `_rowId` so v-for keys survive
      // deletion; assert key/value pairs ignoring the auxiliary id.
      const rows = wrapper.vm.values.sourceParameters.map(
        ({ key, value }) => ({ key, value }),
      );
      expect(rows).toEqual([
        { key: 'bsn', value: '$bsn' },
        { key: 'peildatum', value: '2025-01-01' },
      ]);
    });

    it('skips non-scalar source.parameter values from the editable rows but preserves them on save', async () => {
      const wrapper = mountSheet();
      const warnings: string[] = [];
      const origWarn = console.warn;
      console.warn = (msg: unknown) => warnings.push(String(msg));

      try {
        await setItem(wrapper, {
          section: 'input',
          index: 0,
          data: {
            name: 'leeftijd',
            source: {
              regulation: 'wet_basisregistratie_personen',
              output: 'leeftijd',
              parameters: {
                bsn: '$bsn',
                nested: { foo: 'bar' }, // unsupported in form editor
              },
            },
          },
        });
      } finally {
        console.warn = origWarn;
      }

      const rows = wrapper.vm.values.sourceParameters.map(({ key }) => key);
      expect(rows).toEqual(['bsn']);
      expect(warnings.some((m) => m.includes('nested'))).toBe(true);

      // The non-scalar key must round-trip on save: the form doesn't
      // render it, but it should still appear in the emitted payload
      // unchanged.
      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(firstSave(events).data.source.parameters).toEqual({
        bsn: '$bsn',
        nested: { foo: 'bar' },
      });
    });

    it('round-trips numeric source.parameter values without stringifying them', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'input',
        index: 0,
        data: {
          name: 'leeftijd',
          source: {
            regulation: 'wet_basisregistratie_personen',
            output: 'leeftijd',
            parameters: { threshold: 42, enabled: true, bsn: '$bsn' },
          },
        },
      });

      // No edits to value cells; save should preserve original types.
      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(firstSave(events).data.source.parameters).toEqual({
        threshold: 42,
        enabled: true,
        bsn: '$bsn',
      });
    });

    it('emits user-typed source.parameter value as a string when the user touches the row', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'input',
        index: 0,
        data: {
          name: 'leeftijd',
          source: {
            regulation: 'wet_basisregistratie_personen',
            output: 'leeftijd',
            parameters: { threshold: 42 },
          },
        },
      });

      // Simulate the user editing the value field.
      // Hydration built a row for `threshold`; without it this throws, as before.
      wrapper.vm.values.sourceParameters[0]!.value = '99';

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      // User-typed value emits as a string - we don't second-guess what
      // they meant, and the save path treats explicit edits as opaque
      // text.
      expect(firstSave(events).data.source.parameters).toEqual({ threshold: '99' });
    });

    it('lets a user-added row override an overflow key with the same name', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'input',
        index: 0,
        data: {
          name: 'leeftijd',
          source: {
            regulation: 'wet_basisregistratie_personen',
            output: 'leeftijd',
            parameters: { complex: { foo: 'bar' } },
          },
        },
      });

      // Hydration skipped `complex`; user adds a new row with the same key.
      wrapper.vm.values.sourceParameters.push({ _rowId: 999, key: 'complex', value: 'replaced' });
      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(firstSave(events).data.source.parameters).toEqual({ complex: 'replaced' });
    });

    it('keeps the source keys it does not edit (article, parameters_from, decisions)', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'input',
        index: 0,
        data: {
          name: 'heroverweging',
          type: 'decision',
          source: {
            article: '$besluit.basis',
            parameters_from: ['$besluit.fields'],
            parameters: { dagen: '$dagen' },
          },
        },
      });
      expect(wrapper.vm.values.type).toBe('decision');

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(firstSave(events).data).toEqual({
        name: 'heroverweging',
        type: 'decision',
        source: {
          article: '$besluit.basis',
          parameters_from: ['$besluit.fields'],
          parameters: { dagen: '$dagen' },
        },
      });
    });

    it('keeps the input keys it does not edit (nullable, description)', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'input',
        index: 0,
        data: {
          name: 'heroverweging',
          type: 'decision',
          nullable: true,
          description: 'het bestreden artikel opnieuw',
          source: { article: '$besluit.basis', parameters_from: ['$besluit.fields'] },
        },
      });

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(firstSave(events).data).toEqual({
        name: 'heroverweging',
        type: 'decision',
        nullable: true,
        description: 'het bestreden artikel opnieuw',
        source: { article: '$besluit.basis', parameters_from: ['$besluit.fields'] },
      });
    });

    it('keeps decisions beside an edited output', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'input',
        index: 0,
        data: { name: 'besluiten', source: { decisions: 'wet_a#1' } },
      });
      wrapper.vm.values.sourceOutput = 'verleend';
      wrapper.vm.save();
      await nextTick();
      expect(firstSave(wrapper.emitted('save')).data.source).toEqual({
        decisions: 'wet_a#1',
        output: 'verleend',
      });
    });

    it('keeps a source that reads only facts', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'input',
        index: 0,
        data: { name: 'vestigingen', source: { facts: 'beleid_school#1#vestiging' } },
      });
      wrapper.vm.save();
      await nextTick();
      expect(firstSave(wrapper.emitted('save')).data.source).toEqual({
        facts: 'beleid_school#1#vestiging',
      });
    });

    it('skips source.parameters rows with empty key on save', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'add-input',
        isNew: true,
      });

      wrapper.vm.values.name = 'foo';
      wrapper.vm.values.sourceRegulation = 'bar';
      wrapper.vm.values.sourceParameters = [
        { key: 'bsn', value: '$bsn' },
        { key: '', value: 'orphan' },
        { key: '   ', value: 'whitespace' },
      ];

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(firstSave(events).data.source.parameters).toEqual({ bsn: '$bsn' });
    });

    it('omits source entirely when no regulation/output/parameters set', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'add-input',
        isNew: true,
      });

      wrapper.vm.values.name = 'foo';
      // Leave source* blank.

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(firstSave(events).data).not.toHaveProperty('source');
    });

    it('drops the source block when the user clears regulation and output, even if overflow params remain', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'input',
        index: 0,
        data: {
          name: 'leeftijd',
          source: {
            regulation: 'wet_basisregistratie_personen',
            output: 'leeftijd',
            parameters: { complex: { foo: 'bar' } },
          },
        },
      });

      // Hydration parked the non-scalar `complex` key in
      // sourceParametersOverflow. The user now clears regulation and
      // output, intending to disable the source binding entirely.
      wrapper.vm.values.sourceRegulation = '';
      wrapper.vm.values.sourceOutput = '';
      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      // No `source` should be emitted: parameters alone don't make a
      // valid source block (the schema requires regulation), and the
      // user explicitly cleared the binding.
      expect(firstSave(events).data).not.toHaveProperty('source');
    });
  });

  describe('output', () => {
    it('emits save with output data', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'output',
        index: 0,
        data: { name: 'heeft_recht', type: 'boolean' },
      });

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(firstSave(events)).toMatchObject({
        section: 'output',
        index: 0,
        data: { name: 'heeft_recht', type: 'boolean' },
      });
    });

    it('preserves type_spec on amount outputs', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'output',
        index: 0,
        data: {
          name: 'hoogte',
          type: 'amount',
          type_spec: { unit: 'eurocent' },
        },
      });

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(firstSave(events).data.type_spec).toEqual({ unit: 'eurocent' });
    });
  });

  // Tests drive the combo-box change handlers directly; filter/search UX lives in nldd-combo-box.
  describe('input - law combo-box flow', () => {
    const mockLaws = [
      { law_id: 'wet_basisregistratie_personen', name: null, display_name: 'Wet basisregistratie personen', source_id: 'local', source_name: 'Local' },
      { law_id: 'wet_op_de_zorgtoeslag', name: null, display_name: 'Wet op de zorgtoeslag', source_id: 'local', source_name: 'Local' },
      { law_id: 'kieswet', name: 'Kieswet', display_name: 'Kieswet', source_id: 'local', source_name: 'Local' },
    ];
    const mockOutputs = [
      { name: 'leeftijd', output_type: 'number', article_number: '2.7', parameters: [{ name: 'bsn', param_type: 'string' }, { name: 'peildatum', param_type: 'date' }] },
      { name: 'heeft_partner', output_type: 'boolean', article_number: '2.8', parameters: [{ name: 'bsn', param_type: 'string' }] },
    ];
    let fetchSpy: MockInstance<typeof fetch>;

    beforeEach(() => {
      fetchSpy = vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
        const urlStr = typeof url === 'string' ? url : url.toString();
        if (urlStr.includes('/api/corpus/laws') && urlStr.includes('/outputs')) {
          return fakeResponse(true, mockOutputs);
        }
        if (urlStr.includes('/api/corpus/laws')) {
          return fakeResponse(true, mockLaws);
        }
        return fakeResponse(false, {});
      });
    });

    afterEach(() => {
      fetchSpy.mockRestore();
    });

    it('onLawComboChange sets sourceRegulation, clears sourceOutput, and fetches outputs', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, { section: 'add-input', isNew: true });
      // Wait for fetchLawsList to complete
      await vi.waitFor(() => expect(wrapper.vm.allLaws.length).toBeGreaterThan(0));

      // sourceOutput is cleared on law change so we can detect it
      wrapper.vm.values.sourceOutput = 'stale';
      await wrapper.vm.onLawComboChange({ detail: { value: 'wet_basisregistratie_personen' } });
      await nextTick();

      expect(wrapper.vm.values.sourceRegulation).toBe('wet_basisregistratie_personen');
      expect(wrapper.vm.values.sourceOutput).toBe('');
      // Outputs are fetched and populated
      await vi.waitFor(() => expect(wrapper.vm.availableOutputs.length).toBeGreaterThan(0));
      expect(wrapper.vm.availableOutputs).toEqual(mockOutputs);
    });

    it('onOutputComboChange auto-populates name, type, and parameters', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, { section: 'add-input', isNew: true });
      await vi.waitFor(() => expect(wrapper.vm.allLaws.length).toBeGreaterThan(0));

      // Pick a law so availableOutputs is populated
      await wrapper.vm.onLawComboChange({ detail: { value: 'wet_basisregistratie_personen' } });
      await vi.waitFor(() => expect(wrapper.vm.availableOutputs.length).toBeGreaterThan(0));

      // Name is empty, type defaults to 'string'
      expect(wrapper.vm.values.name).toBe('');
      wrapper.vm.onOutputComboChange({ detail: { value: 'leeftijd' } });

      expect(wrapper.vm.values.sourceOutput).toBe('leeftijd');
      expect(wrapper.vm.values.name).toBe('leeftijd');
      expect(wrapper.vm.values.type).toBe('number');
      // Parameters pre-populated from the selected output
      const paramKeys = wrapper.vm.values.sourceParameters.map(p => p.key);
      expect(paramKeys).toEqual(['bsn', 'peildatum']);
    });

    it('onOutputComboChange does NOT overwrite user-edited name', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, { section: 'add-input', isNew: true });
      await vi.waitFor(() => expect(wrapper.vm.allLaws.length).toBeGreaterThan(0));

      await wrapper.vm.onLawComboChange({ detail: { value: 'wet_basisregistratie_personen' } });
      await vi.waitFor(() => expect(wrapper.vm.availableOutputs.length).toBeGreaterThan(0));

      // User already set a custom name
      wrapper.vm.values.name = 'custom_input';
      wrapper.vm.onOutputComboChange({ detail: { value: 'leeftijd' } });
      expect(wrapper.vm.values.name).toBe('custom_input');
      // Type is still updated - it's determined by the source output, not the user
      expect(wrapper.vm.values.type).toBe('number');
    });

    it('onOutputComboChange resets parameter rows even when switching to an output with fewer params', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, { section: 'add-input', isNew: true });
      await vi.waitFor(() => expect(wrapper.vm.allLaws.length).toBeGreaterThan(0));
      await wrapper.vm.onLawComboChange({ detail: { value: 'wet_basisregistratie_personen' } });
      await vi.waitFor(() => expect(wrapper.vm.availableOutputs.length).toBeGreaterThan(0));

      // First select leeftijd (2 params), then switch to heeft_partner (1 param).
      // Stale rows from the first selection must not leak into the second.
      wrapper.vm.onOutputComboChange({ detail: { value: 'leeftijd' } });
      expect(wrapper.vm.values.sourceParameters.map(p => p.key)).toEqual(['bsn', 'peildatum']);
      wrapper.vm.onOutputComboChange({ detail: { value: 'heeft_partner' } });
      expect(wrapper.vm.values.sourceParameters.map(p => p.key)).toEqual(['bsn']);
    });

    it('shows the display names of an existing input in the combo-boxes once laws and outputs load', async () => {
      // The combo-box only derives its shown text when `value` changes, and
      // the menu items arrive later from the fetch, so the sheet sets `text`
      // itself.
      const wrapper = mountSheet();
      await setItem(wrapper, {
        section: 'input',
        key: 'leeftijd',
        data: { name: 'leeftijd', type: 'number', source: { regulation: 'wet_basisregistratie_personen', output: 'leeftijd' } },
      });
      await vi.waitFor(() => expect(wrapper.vm.availableOutputs.length).toBeGreaterThan(0));
      await nextTick();

      const lawBox = wrapper.find('[data-testid="law-combo-box"]').element as HTMLElement & { text?: string };
      const outputBox = wrapper.find('[data-testid="output-combo-box"]').element as HTMLElement & { text?: string };
      expect(lawBox.text).toBe('Wet basisregistratie personen');
      expect(outputBox.text).toBe('leeftijd (number)');
    });

    it('allLaws is exposed for the combo-box menu', async () => {
      // The nldd-combo-box does its own search/filter via menu-item aliases;
      // this component just needs to expose the full law list and a stable
      // displayName helper. (filteredLaws / lawSearchQuery are gone.)
      const wrapper = mountSheet();
      await setItem(wrapper, { section: 'add-input', isNew: true });
      await vi.waitFor(() => expect(wrapper.vm.allLaws.length).toBeGreaterThan(0));

      const ids = wrapper.vm.allLaws.map((l) => l.law_id);
      expect(ids).toEqual(['wet_basisregistratie_personen', 'wet_op_de_zorgtoeslag', 'kieswet']);
    });

    it('save emits correct data after combo-box select flow', async () => {
      const wrapper = mountSheet();
      await setItem(wrapper, { section: 'add-input', isNew: true });
      await vi.waitFor(() => expect(wrapper.vm.allLaws.length).toBeGreaterThan(0));

      await wrapper.vm.onLawComboChange({ detail: { value: 'wet_basisregistratie_personen' } });
      await vi.waitFor(() => expect(wrapper.vm.availableOutputs.length).toBeGreaterThan(0));
      wrapper.vm.onOutputComboChange({ detail: { value: 'leeftijd' } });

      wrapper.vm.save();
      await nextTick();
      const events = wrapper.emitted('save');
      expect(firstSave(events)).toMatchObject({
        section: 'add-input',
        data: {
          name: 'leeftijd',
          type: 'number',
          source: {
            regulation: 'wet_basisregistratie_personen',
            output: 'leeftijd',
          },
        },
      });
    });

    it('displayName resolves display_name, then name, then title-cased law_id', () => {
      const wrapper = mountSheet();
      const dn = wrapper.vm.displayName;
      expect(dn({ display_name: 'Foo', name: 'Bar', law_id: 'baz' })).toBe('Foo');
      expect(dn({ display_name: null, name: 'Bar', law_id: 'baz' })).toBe('Bar');
      expect(dn({ display_name: null, name: null, law_id: 'wet_brp' })).toBe('Wet Brp');
    });
  });
});
