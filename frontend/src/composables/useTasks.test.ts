import { describe, it, expect, vi, beforeEach } from 'vitest';

// Route the composable's network leg through a controllable spy. The
// factory closes over `apiFetch` so it survives `vi.resetModules()`. Only
// `apiFetch` is stubbed - useTasks.js is the only file under test that
// imports from '@regelrecht/frontend-shared', so the other package exports
// (useAuth, useColorScheme, ...) don't need stubs here.
const apiFetch = vi.fn();
vi.mock('@regelrecht/frontend-shared', () => ({ apiFetch: (...a: unknown[]) => apiFetch(...a) }));

describe('useTasks', () => {
  beforeEach(() => {
    vi.resetModules();
    apiFetch.mockReset();
  });

  it('laadt open taken en gebruikt open_count van de server', async () => {
    apiFetch.mockResolvedValue({
      status: 200,
      json: async () => ({ tasks: [{ id: 't1', task_type: 'job_review', title: 'x' }], open_count: 5 }),
    });
    const { useTasks } = await import('./useTasks');
    const { tasks, openCount, refresh } = useTasks();
    await refresh();
    expect(apiFetch).toHaveBeenCalledWith('/api/tasks', expect.anything());
    expect(tasks.value).toHaveLength(1);
    expect(openCount.value).toBe(5);
  });

  it('parseert running uit de response', async () => {
    apiFetch.mockResolvedValue({
      status: 200,
      json: async () => ({
        tasks: [],
        open_count: 0,
        running: [{ job_id: 'j1', law_id: 'test_wet', status: 'pending' }],
      }),
    });
    const { useTasks } = await import('./useTasks');
    const { running, refresh } = useTasks();
    await refresh();
    expect(running.value).toHaveLength(1);
    // Length checked on the line above.
    expect(running.value[0]!.law_id).toBe('test_wet');
  });

  it('valt terug op een lege running-lijst zonder het veld in de response', async () => {
    apiFetch.mockResolvedValue({
      status: 200,
      json: async () => ({ tasks: [], open_count: 0 }),
    });
    const { useTasks } = await import('./useTasks');
    const { running, refresh } = useTasks();
    await refresh();
    expect(running.value).toEqual([]);
  });

  it('houdt stale taken vast bij een poll-fout', async () => {
    apiFetch.mockResolvedValueOnce({
      status: 200,
      json: async () => ({ tasks: [{ id: 't1' }], open_count: 1 }),
    });
    const { useTasks } = await import('./useTasks');
    const { tasks, refresh } = useTasks();
    await refresh();
    apiFetch.mockRejectedValueOnce(new Error('netwerk'));
    await refresh();
    expect(tasks.value).toHaveLength(1);
  });

  it('resolve roept het resolve-endpoint en herlaadt', async () => {
    apiFetch.mockResolvedValue({ status: 200, json: async () => ({ tasks: [], open_count: 0 }) });
    const { useTasks } = await import('./useTasks');
    const { resolveTask } = useTasks();
    await resolveTask('t1', 'dismissed');
    expect(apiFetch).toHaveBeenCalledWith(
      '/api/tasks/t1/resolve',
      expect.objectContaining({ method: 'POST' })
    );
  });

  it('requestEnrich meldt alreadyRunning bij 409 en tooMany bij 429', async () => {
    apiFetch.mockResolvedValueOnce({ status: 409 });
    const { useTasks } = await import('./useTasks');
    const { requestEnrich } = useTasks();
    const r1 = await requestEnrich('traject-abcd1234', 'test_wet');
    expect(r1.alreadyRunning).toBe(true);
    apiFetch.mockResolvedValueOnce({ status: 429 });
    const r2 = await requestEnrich('traject-abcd1234', 'test_wet');
    expect(r2.tooMany).toBe(true);
  });

  it('requestEnrich sends no body without a provider, so the deployment default runs', async () => {
    apiFetch.mockResolvedValueOnce({ status: 202 });
    const { useTasks } = await import('./useTasks');
    const { requestEnrich } = useTasks();
    await requestEnrich('traject-abcd1234', 'test_wet');
    const init = apiFetch.mock.calls.find(([url]) => String(url).endsWith('/enrich'))?.[1];
    expect(init?.body).toBeUndefined();
  });

  it('requestEnrich sends the chosen provider as JSON', async () => {
    apiFetch.mockResolvedValueOnce({ status: 202 });
    const { useTasks } = await import('./useTasks');
    const { requestEnrich } = useTasks();
    await requestEnrich('traject-abcd1234', 'test_wet', 'opencode');
    const init = apiFetch.mock.calls.find(([url]) => String(url).endsWith('/enrich'))?.[1];
    expect(init?.headers).toEqual({ 'Content-Type': 'application/json' });
    expect(JSON.parse(init?.body as string)).toEqual({ provider: 'opencode' });
  });

  it('useTaskActions triggert geen fetch bij aanroep (geen poll voor niet-badge/sheet-callers)', async () => {
    const { useTaskActions } = await import('./useTasks');
    const actions = useTaskActions();
    // useTasks() plant zijn initial load een microtask-tick later (zie de
    // comment daar) - flush een paar ticks om zeker te zijn dat
    // useTaskActions() zelf geen vergelijkbare refresh plant.
    await Promise.resolve();
    await Promise.resolve();
    expect(apiFetch).not.toHaveBeenCalled();
    expect(actions).toEqual(
      expect.objectContaining({
        fetchTask: expect.any(Function),
        resolveTask: expect.any(Function),
        requestEnrich: expect.any(Function),
        refresh: expect.any(Function),
      })
    );
  });
});
