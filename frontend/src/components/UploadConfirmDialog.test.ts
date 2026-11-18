// De uploadbevestiging: welk verhaal krijgt de gebruiker per formaat, en wanneer
// is "Uploaden" pas een zinnige knop. De serverindeling wordt gemockt - de
// klassering zelf is elders getest (lib/uploadFormats.test.js); hier gaat het om
// wat de dialoog ermee doet.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import type { VueWrapper } from '@vue/test-utils';

const formats = {
  passthrough: ['md', 'markdown'],
  deterministic: ['docx', 'pdf'],
};
const loadUploadFormats = vi.fn();
vi.mock('../lib/uploadFormats', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../lib/uploadFormats')>();
  return { ...actual, loadUploadFormats: () => loadUploadFormats() };
});

import UploadConfirmDialog from './UploadConfirmDialog.vue';

beforeEach(() => {
  loadUploadFormats.mockResolvedValue(formats);
});

async function mountFor(filename: string) {
  const wrapper = mount(UploadConfirmDialog, {
    props: { file: { name: filename } },
    global: { stubs: { teleport: true } },
  });
  // De watcher vuurt direct bij mounten; de formaatindeling landt een
  // microtask later.
  await flushPromises();
  return wrapper;
}

type Wrapper = VueWrapper<InstanceType<typeof UploadConfirmDialog>>;
const checkbox = (w: Wrapper) => w.find('[data-testid="upload-confirm-llm"]');
const submit = (w: Wrapper) => w.find('[data-testid="upload-confirm-submit"]');
const supportingText = (w: Wrapper) => w.find('nldd-modal-dialog').attributes('supporting-text');

describe('UploadConfirmDialog', () => {
  it('toont de bestandsnaam', async () => {
    const wrapper = await mountFor('rapport.docx');
    expect(wrapper.html()).toContain('rapport.docx');
  });

  it('markdown: geen vinkje, en de melding dat er niets omgezet hoeft', async () => {
    const wrapper = await mountFor('notitie.md');
    expect(checkbox(wrapper).exists()).toBe(false);
    expect(supportingText(wrapper)).toContain('direct opgeslagen');
    expect(submit(wrapper).attributes('disabled')).toBeUndefined();
  });

  it('formaat met converter: vinkje standaard uit, uploaden blijft mogelijk', async () => {
    const wrapper = await mountFor('rapport.docx');
    expect(checkbox(wrapper).exists()).toBe(true);
    expect(checkbox(wrapper).attributes('checked')).toBeUndefined();
    expect(supportingText(wrapper)).toContain('zonder AI');
    expect(submit(wrapper).attributes('disabled')).toBeUndefined();
  });

  it('formaat zonder converter: uploaden pas mogelijk mét het vinkje aan', async () => {
    const wrapper = await mountFor('brief.doc');
    expect(checkbox(wrapper).exists()).toBe(true);
    expect(supportingText(wrapper)).toContain('alleen met AI');
    expect(submit(wrapper).attributes('disabled')).toBeDefined();

    await checkbox(wrapper).trigger('change', { detail: { checked: true } });
    expect(submit(wrapper).attributes('disabled')).toBeUndefined();
  });

  it('bevestigen meldt de gemaakte keuze', async () => {
    const wrapper = await mountFor('rapport.docx');
    await submit(wrapper).trigger('click');
    expect(wrapper.emitted('confirm')).toBeDefined();
    expect(wrapper.emitted('confirm')![0]).toEqual([{ allowLlm: false }]);

    await checkbox(wrapper).trigger('change', { detail: { checked: true } });
    await submit(wrapper).trigger('click');
    expect(wrapper.emitted('confirm')![1]).toEqual([{ allowLlm: true }]);
  });

  it('een niet-toegestane bevestiging levert geen upload op', async () => {
    const wrapper = await mountFor('brief.doc');
    await submit(wrapper).trigger('click');
    expect(wrapper.emitted('confirm')).toBeUndefined();
  });

  it('zonder serverindeling blijft het vinkje uit staan en de knop bruikbaar', async () => {
    // Behoedzaam maar niet blokkerend: er wordt geen toestemming gegeven, en de
    // backend weigert alsnog wanneer het formaat AI nodig heeft.
    loadUploadFormats.mockResolvedValue(null);
    const wrapper = await mountFor('rapport.docx');
    expect(checkbox(wrapper).exists()).toBe(true);
    expect(checkbox(wrapper).attributes('checked')).toBeUndefined();
    expect(submit(wrapper).attributes('disabled')).toBeUndefined();
    expect(supportingText(wrapper)).toContain('niet vaststellen');
  });

  it('vergeet een eerder gegeven toestemming bij een volgend bestand', async () => {
    const wrapper = await mountFor('brief.doc');
    await checkbox(wrapper).trigger('change', { detail: { checked: true } });
    expect(submit(wrapper).attributes('disabled')).toBeUndefined();

    await wrapper.setProps({ file: { name: 'andere.doc' } });
    await flushPromises();
    expect(submit(wrapper).attributes('disabled')).toBeDefined();
  });
});
