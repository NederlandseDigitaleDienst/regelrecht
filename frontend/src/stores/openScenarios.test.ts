import { describe, it, expect, beforeEach } from 'vitest';
import { useOpenScenariosStore } from './openScenarios';
import { res, stubFetch } from '../test-utils/fetchStubs';

const FEATURE = `Feature: Basis

  Scenario: Een
    Given parameter "inkomen" is 1
    When I evaluate "x" of "wet_a"
`;

const A = { trajectRef: 'tr-aaaaaaaa', lawId: 'wet_a', filename: 'basis.feature' };
const B = { trajectRef: 'tr-bbbbbbbb', lawId: 'wet_a', filename: 'basis.feature' };

beforeEach(() => {
  stubFetch(async () => res({ body: FEATURE }));
});

async function dirtyFile(file: typeof A) {
  const doc = useOpenScenariosStore().open(file);
  await doc.load();
  doc.setValues(0, { ...doc.values.value[0]!, parameterValues: { inkomen: '2' } });
  return doc;
}

describe('openScenarios store', () => {
  it('keeps one open copy per traject, regelwerk and file', () => {
    const store = useOpenScenariosStore();
    expect(store.open(A)).toBe(store.open({ ...A }));
    expect(store.open(A)).not.toBe(store.open(B));
  });

  it('keeps the edits of a file for whoever opens it next', async () => {
    const doc = await dirtyFile(A);
    expect(useOpenScenariosStore().open(A).dirty.value).toBe(true);
    expect(useOpenScenariosStore().find(A)).toBe(doc);
  });

  it('names the files with unsaved edits per traject', async () => {
    const store = useOpenScenariosStore();
    await dirtyFile(A);
    store.open(B);
    expect(store.dirtyIn(A.trajectRef).map((d) => d.file)).toEqual([A]);
    expect(store.dirtyIn(B.trajectRef)).toEqual([]);
    expect(store.anyDirty()).toBe(true);
  });

  it('forgets a closed file and every file of a closed traject', async () => {
    const store = useOpenScenariosStore();
    await dirtyFile(A);
    store.open(B);
    store.close(B);
    expect(store.find(B)).toBeUndefined();
    store.closeTraject(A.trajectRef);
    expect(store.find(A)).toBeUndefined();
    expect(store.anyDirty()).toBe(false);
  });
});
