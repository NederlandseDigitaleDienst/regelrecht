// @vitest-environment node
// Activating the app at a RegelHet and calling its citizen's routes, against
// a RegelHet in memory that checks what the server checks
// (testondersteuning/nepRegelhet.ts).
import { describe, expect, it } from 'vitest';
import { kernSessie } from '../tauri/kern';
import { leesConfig } from './config';
import { Burgerapp } from './kern';
import { geheugenOpslag } from './opslag';
import { nepRegelhet } from './testondersteuning/nepRegelhet';

async function opzet() {
  const server = await nepRegelhet();
  const opslag = geheugenOpslag();
  const regelhets = leesConfig(JSON.stringify({ regelhets: [{ id: 'parkeerkaart', naam: 'Parkeerkaart', adres: server.adres }] }));
  const app = new Burgerapp(regelhets, opslag, server.haal);
  return { server, opslag, app };
}

describe('the app at a RegelHet', () => {
  it('is not activated before it activates, and keeps nothing', async () => {
    const { app, opslag } = await opzet();
    const s = await app.status('parkeerkaart');
    expect(s.holder).toBeNull();
    expect(s.key_storage).toEqual({ kind: 'browser' });
    expect(s.hardware_backed).toBe(false);
    expect(await opslag.lees('parkeerkaart')).toBeNull();
  });

  it('activates for a test citizen: a key it cannot read, a credential bound to it, and a session', async () => {
    const { app, opslag, server } = await opzet();
    const s = await app.activeren('parkeerkaart', '999990032');
    expect(s.holder).toEqual({ bsn: '999990032', name: 'Test Burger' });
    const bewaard = await opslag.lees('parkeerkaart');
    expect(bewaard?.sleutel.privateKey.extractable).toBe(false);
    expect(bewaard?.credential).toMatch(/^[\w-]+\.[\w-]+\.[\w-]+$/);
    expect(server.sessies()).toBe(1);
    expect(await app.duimafdruk('parkeerkaart')).toHaveLength(43);
  });

  it('refuses a number outside the demo and a RegelHet it does not know', async () => {
    const { app } = await opzet();
    await expect(app.activeren('parkeerkaart', '123456782')).rejects.toMatchObject({ kind: 'input' });
    await expect(app.activeren('elders', '999990032')).rejects.toMatchObject({ kind: 'not_configured' });
  });

  it('says so when the server offers no demo Stelsel, and stays not activated', async () => {
    const { app, server } = await opzet();
    server.zonderDemo = true;
    await expect(app.activeren('parkeerkaart', '999990032')).rejects.toMatchObject({ kind: 'no_demo' });
    expect((await app.status('parkeerkaart')).holder).toBeNull();
  });

  it('sends the screens’ calls encrypted under the session, the idempotency key inside', async () => {
    const { app, server } = await opzet();
    const gezien: Record<string, unknown>[] = [];
    server.routes.casus = () => ({ status: 200, body: { id: 'gehandicaptenparkeerkaart' } });
    server.routes.indieningen = (v, bsn) => (gezien.push(v), { status: 200, body: { id: 'g1', bsn } });
    await app.activeren('parkeerkaart', '999990032');
    const sessie = kernSessie(app.invoke(() => 'parkeerkaart'));
    expect(await sessie.haal('/api/casus')).toEqual({ id: 'gehandicaptenparkeerkaart' });
    expect(await sessie.stuur('POST', '/api/indieningen', { velden: { a: 1 } }, 'k1')).toEqual({ id: 'g1', bsn: '999990032' });
    expect(gezien).toEqual([{ velden: { a: 1 }, idempotency_key: 'k1' }]);
  });

  it('starts a new session once when the server ended the old one, and sends the request again', async () => {
    const { app, server } = await opzet();
    server.routes.dossier = () => ({ status: 200, body: { grammen: [] } });
    await app.activeren('parkeerkaart', '999990032');
    server.sessiesVerlopen();
    expect(await app.roep('parkeerkaart', 'dossier', {})).toEqual({ grammen: [] });
    expect(server.sessies()).toBe(2);
  });

  it('gives the service’s refusal to the screen with its status and reason', async () => {
    const { app, server } = await opzet();
    server.routes.indieningen = () => ({ status: 422, body: { fout: 'het adres ontbreekt' } });
    await app.activeren('parkeerkaart', '999990032');
    const sessie = kernSessie(app.invoke(() => 'parkeerkaart'));
    await expect(sessie.stuur('POST', '/api/indieningen', {}, 'k')).rejects.toMatchObject({ status: 422, message: 'het adres ontbreekt' });
  });

  it('forgets a credential the RegelHet refuses, so the citizen activates again instead of looping', async () => {
    const { app, server, opslag } = await opzet();
    await app.activeren('parkeerkaart', '999990032');
    server.weigertCredentials = true;
    server.sessiesVerlopen();
    await expect(app.roep('parkeerkaart', 'casus', {})).rejects.toMatchObject({ kind: 'credential_refused' });
    expect((await app.status('parkeerkaart')).holder).toBeNull();
    const bewaard = await opslag.lees('parkeerkaart');
    expect(bewaard?.credential).toBeNull();
    expect(bewaard?.sleutel).toBeDefined();
  });

  it('forgets key and credential when the activation is withdrawn, and asks to activate again', async () => {
    const { app, opslag } = await opzet();
    await app.activeren('parkeerkaart', '999990032');
    await app.intrekken('parkeerkaart');
    expect(await opslag.lees('parkeerkaart')).toBeNull();
    await expect(app.roep('parkeerkaart', 'casus', {})).rejects.toMatchObject({ kind: 'not_activated' });
  });

  it('activates at each RegelHet with a key of its own', async () => {
    const a = await nepRegelhet('https://a.example.org');
    const b = await nepRegelhet('https://b.example.org');
    const haal = (url: string, init?: RequestInit) => (url.startsWith(a.adres) ? a.haal(url, init) : b.haal(url, init));
    const regelhets = leesConfig(JSON.stringify({ regelhets: [{ id: 'a', naam: 'A', adres: a.adres }, { id: 'b', naam: 'B', adres: b.adres }] }));
    const app = new Burgerapp(regelhets, geheugenOpslag(), haal);
    await app.activeren('a', '999990032');
    await app.activeren('b', '999990020');
    expect((await app.status('a')).holder?.bsn).toBe('999990032');
    expect((await app.status('b')).holder?.bsn).toBe('999990020');
    expect(await app.duimafdruk('a')).not.toBe(await app.duimafdruk('b'));
  });
});

describe('the list of RegelHets', () => {
  it('takes an id, a name and an address, and the demo OIN by default', () => {
    expect(leesConfig('{"regelhets":[{"id":"p","naam":"P","adres":"https://p.example.org/"}]}')).toEqual([
      { id: 'p', naam: 'P', adres: 'https://p.example.org', oin: '00000099000000000001' },
    ]);
  });

  it('refuses plain http elsewhere than localhost, a double id and a missing list', () => {
    expect(() => leesConfig('{"regelhets":[{"id":"p","naam":"P","adres":"http://p.example.org"}]}')).toThrow(/https/);
    expect(() => leesConfig('{"regelhets":[{"id":"p","naam":"P","adres":"https://a.org"},{"id":"p","naam":"Q","adres":"https://b.org"}]}')).toThrow(/twee keer/);
    expect(() => leesConfig('{}')).toThrow(/regelhets/);
  });
});
