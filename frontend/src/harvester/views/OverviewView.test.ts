import { shallowMount } from '@vue/test-utils';
import { describe, it, expect, afterEach, vi } from 'vitest';
import { ref } from 'vue';
import OverviewView from './OverviewView.vue';
import DataTable from '../components/DataTable.vue';
import DailyJobsChart from '../components/DailyJobsChart.vue';
import type { DashboardStats } from '../composables/useDashboardStats';

// DataTable is a generic component (`<script setup generic>`), whose props
// test-utils cannot type; read them as a plain record.
function tableProps(table: { props(): unknown }): Record<string, unknown> {
  return table.props() as Record<string, unknown>;
}

// One bucket per job_type, as the API serves them. `document_convert` is in
// here on purpose: the page used to hardcode harvest/enrich and drop the rest.
const STATS: DashboardStats = {
  jobs: {
    total: 1211,
    by_type: { document_convert: 7, enrich: 362, harvest: 842 },
    by_status: { pending: 13, processing: 3, completed: 1186, failed: 9 },
    by_type_status: {
      document_convert: { pending: 1, processing: 0, completed: 6, failed: 0 },
      enrich: { pending: 7, processing: 1, completed: 350, failed: 4 },
      harvest: { pending: 5, processing: 2, completed: 830, failed: 5 },
    },
  },
  executed: {
    today: { total: 49, document_convert: 2, enrich: 17, harvest: 30 },
    last_7d: { total: 217, document_convert: 7, enrich: 70, harvest: 140 },
  },
  open_untranslatables: 23,
  daily: [
    {
      date: '2026-07-06',
      document_convert: { added: 1, succeeded: 1, failed: 0 },
      enrich: { added: 2, succeeded: 2, failed: 0 },
      harvest: { added: 4, succeeded: 3, failed: 1 },
    },
    {
      date: '2026-07-07',
      document_convert: { added: 0, succeeded: 0, failed: 0 },
      enrich: { added: 0, succeeded: 1, failed: 1 },
      harvest: { added: 1, succeeded: 0, failed: 0 },
    },
  ],
  recent_failures: [
    {
      id: 'job-1',
      law_id: 'BWBR0001',
      job_type: 'enrich',
      failed_at: '2026-07-03T10:00:00Z',
      error: 'job timed out after 300s',
    },
  ],
};

const openDetail = vi.fn();
const closeDetail = vi.fn();

// Per-mount stats value - tests can swap it to mount with a variant payload.
let currentStats = STATS;

vi.mock('../composables/useDashboardStats', () => ({
  useDashboardStats: () => ({
    stats: ref(currentStats),
    loading: ref(false),
    error: ref(null),
  }),
}));

vi.mock('../composables/useJobDetail', () => ({
  useJobDetail: () => ({
    job: ref(null),
    isOpen: ref(false),
    open: openDetail,
    close: closeDetail,
  }),
}));

afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe('OverviewView', () => {
  it('renders the top KPI totals (nl-NL formatted)', () => {
    const w = shallowMount(OverviewView);
    const text = w.text();
    expect(text).toContain('1.211'); // jobs total, som over alle typen
    expect(text).toContain('Jobs totaal');
    expect(text).toContain('23'); // open untranslatables
    expect(text).toContain('Open untranslatables');
  });

  it('renders harvest and enrich as two separate per-type panels', () => {
    const w = shallowMount(OverviewView);
    // Values live in overline/title slots (light DOM) or on nldd-text-cell
    // `text` attributes (shadow DOM); w.html() covers both.
    const html = w.html();
    // Per-type headings + totals, for every type the API returned
    expect(html).toContain('Harvest');
    expect(html).toContain('842');
    expect(html).toContain('Enrich');
    expect(html).toContain('362');
    expect(html).toContain('Document convert');
    expect(html).toContain('text="6"');
    // Per-type executed counts (harvest today 30 / week 140, enrich today 17 / week 70)
    expect(html).toContain('Uitgevoerd vandaag');
    expect(html).toContain('Afgelopen 7 dagen');
    expect(html).toContain('text="30"');
    expect(html).toContain('text="17"');
    // Per-type status counts come from by_type_status, not the combined by_status.
    // harvest.completed = 830, enrich.completed = 350
    expect(html).toContain('text="830"');
    expect(html).toContain('text="350"');
  });

  it('renders one daily chart per job type with mapped entries', () => {
    const w = shallowMount(OverviewView);
    const charts = w.findAllComponents(DailyJobsChart);
    expect(charts).toHaveLength(3);

    // Charts sit in the right column of their type's half/half section, in the
    // key order the API used (document_convert, enrich, harvest).
    // Non-null: the length was just asserted.
    expect(charts[0]!.attributes('slot')).toBe('right');
    expect(charts[0]!.props('entries')).toEqual([
      { date: '2026-07-06', added: 1, succeeded: 1, failed: 0 },
      { date: '2026-07-07', added: 0, succeeded: 0, failed: 0 },
    ]);
    expect(charts[2]!.props('entries')).toEqual([
      { date: '2026-07-06', added: 4, succeeded: 3, failed: 1 },
      { date: '2026-07-07', added: 1, succeeded: 0, failed: 0 },
    ]);

    expect(charts[1]!.props('entries')).toEqual([
      { date: '2026-07-06', added: 2, succeeded: 2, failed: 0 },
      { date: '2026-07-07', added: 0, succeeded: 1, failed: 1 },
    ]);
  });

  it('hides the charts (but keeps the panels) when the API has no daily block', () => {
    const stripped = { ...STATS };
    delete stripped.daily;
    currentStats = stripped;
    try {
      const w = shallowMount(OverviewView);
      expect(w.findAllComponents(DailyJobsChart)).toHaveLength(0);
      expect(w.html()).toContain('Harvest');
    } finally {
      currentStats = STATS;
    }
  });

  it('wires the failures DataTable with the recent_failures data', () => {
    const w = shallowMount(OverviewView);
    const table = w.findComponent(DataTable);
    expect(tableProps(table).data).toEqual(STATS.recent_failures);
    expect(tableProps(table).clickableRows).toBe(true);
    // No sortOptions passed → the table toolbar stays empty.
    expect(tableProps(table).sortOptions).toBeNull();
  });

  it('fetches the full job and opens the detail panel on row click', async () => {
    const fullJob = { id: 'job-1', status: 'failed', result: { error: 'job timed out after 300s' } };
    vi.stubGlobal(
      'fetch',
      vi.fn(() =>
        Promise.resolve(
          new Response(JSON.stringify(fullJob), {
            status: 200,
            headers: { 'content-type': 'application/json' },
          }),
        ),
      ),
    );

    const w = shallowMount(OverviewView);
    w.findComponent(DataTable).vm.$emit('row-click', STATS.recent_failures[0]);

    await vi.waitFor(() => expect(openDetail).toHaveBeenCalled());
    expect(openDetail).toHaveBeenCalledWith(fullJob);
  });
});
