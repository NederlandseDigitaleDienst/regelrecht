// A structural gate on the three flow datasets, in the same spirit as
// script/cross-law-integriteit.py: a reference that points at nothing is a
// modeling error and turns the test red.
//
// This app has no other tests. The components render whatever is in the data
// and complain about nothing: a connection to a stage that does not exist
// draws nothing, and a stage on a branch that does not exist gets no color.
// Both stay invisible until someone holds the diagram next to its intent.

import { expect, test } from 'vitest';
import * as simple from './flowDataSimple';
import * as advanced from './flowDataAdvanced';
import * as woo from './flowDataWoo';
import type { Branch, Connection, Stage } from './types';

const loaded: { file: string; branches: Branch[]; stages: Stage[]; connections: Connection[] }[] = [
  { file: 'flowDataSimple.ts', ...simple },
  { file: 'flowDataAdvanced.ts', ...advanced },
  { file: 'flowDataWoo.ts', ...woo },
];

test('every dataset has branches, stages and connections', () => {
  // Without this assertion a green run says nothing about a dataset that
  // imports empty by accident.
  for (const { file, branches, stages, connections } of loaded) {
    expect(branches.length, `${file} has no branches`).toBeGreaterThan(0);
    expect(stages.length, `${file} has no stages`).toBeGreaterThan(0);
    expect(connections.length, `${file} has no connections`).toBeGreaterThan(0);
  }
});

test('stage ids are unique within a dataset', () => {
  for (const { file, stages } of loaded) {
    const seen = new Set<string>();
    for (const s of stages) {
      expect(seen.has(s.id), `${file}: stage id "${s.id}" occurs more than once`).toBe(false);
      seen.add(s.id);
    }
  }
});

test('branch ids are unique within a dataset', () => {
  for (const { file, branches } of loaded) {
    const seen = new Set<string>();
    for (const b of branches) {
      expect(seen.has(b.id), `${file}: branch id "${b.id}" occurs more than once`).toBe(false);
      seen.add(b.id);
    }
  }
});

test('every stage sits on a branch that exists', () => {
  for (const { file, branches, stages } of loaded) {
    const ids = new Set(branches.map((b) => b.id));
    for (const s of stages) {
      expect(ids.has(s.branch), `${file}: stage "${s.id}" sits on unknown branch "${s.branch}"`).toBe(true);
    }
  }
});

test('every connection points at an existing stage on both ends', () => {
  for (const { file, stages, connections } of loaded) {
    const ids = new Set(stages.map((s) => s.id));
    for (const c of connections) {
      expect(ids.has(c.from), `${file}: connection from unknown stage "${c.from}"`).toBe(true);
      expect(ids.has(c.to), `${file}: connection to unknown stage "${c.to}"`).toBe(true);
    }
  }
});

test('no stage points at itself', () => {
  for (const { file, connections } of loaded) {
    for (const c of connections) {
      expect(c.from, `${file}: stage "${c.from}" points at itself`).not.toBe(c.to);
    }
  }
});

test('a stage without an incoming connection sits on the trunk', () => {
  // FlowDiagram fixes rows along the incoming connections; without one, a
  // stage falls back to the global order. On the trunk that is intended (main
  // runs on next to the proposal); in any other column it means the branch-off
  // to that stage is missing.
  for (const { file, stages, connections } of loaded) {
    const reachable = new Set(connections.map((c) => c.to));
    const floating = stages.filter((s) => !reachable.has(s.id) && s.col !== 0);
    expect(
      floating.map((s) => s.id),
      `${file}: stage(s) off the trunk without an incoming connection`,
    ).toEqual([]);
  }
});

test('every stage has a numeric step and col', () => {
  for (const { file, stages } of loaded) {
    for (const s of stages) {
      expect(typeof s.step, `${file}: stage "${s.id}" has no numeric step`).toBe('number');
      expect(typeof s.col, `${file}: stage "${s.id}" has no numeric col`).toBe('number');
    }
  }
});
