/**
 * De bron op de vite-dev-server: de route bron/spoorzone die achter het poc-portaal het portaal
 * zelf serveert (packages/poc-portal/src/bron.rs), hier uit de lokale map in POC_BRON_SPOORZONE,
 * in dezelfde vorm: { naam, versie, bestanden: { <pad>: <tekst> } }, alleen YAML-bestanden.
 * Zonder die variabele antwoordt de route 503, zoals het portaal zonder token, en toont de app
 * dat de gegevens niet beschikbaar zijn. Alleen een map, geen manifest.
 */
import { existsSync, readdirSync, readFileSync } from 'fs';
import { join, relative, resolve } from 'path';

/** De env-var met de lokale map van bron `naam`: spoorzone -> POC_BRON_SPOORZONE. */
export const lokaleEnv = (naam) => `POC_BRON_${naam.toUpperCase().replace(/-/g, '_')}`;

/** Elk YAML-bestand onder `dir`, per pad onder `dir`. */
export function bestandenUitMap(dir) {
  const uit = {};
  const loop = (map) => {
    for (const entry of readdirSync(map, { withFileTypes: true })) {
      const pad = join(map, entry.name);
      if (entry.isDirectory()) loop(pad);
      else if (/\.ya?ml$/.test(entry.name)) uit[relative(dir, pad).split('\\').join('/')] = readFileSync(pad, 'utf8');
    }
  };
  loop(dir);
  return uit;
}

/** Het antwoord op bron/<naam>: [status, content-type, tekst]. */
export function bronAntwoord(naam, env = process.env) {
  const map = env[lokaleEnv(naam)];
  if (!map) return [503, 'text/plain; charset=utf-8', `bron niet beschikbaar: ${lokaleEnv(naam)} is niet gezet`];
  const dir = resolve(map);
  if (!existsSync(dir)) return [502, 'text/plain; charset=utf-8', `${dir} bestaat niet`];
  const bestanden = bestandenUitMap(dir);
  if (!Object.keys(bestanden).length) return [502, 'text/plain; charset=utf-8', `geen YAML-bestanden in ${dir}`];
  return [200, 'application/json', JSON.stringify({ naam, versie: 'lokaal', bestanden })];
}

export function bronDev(base = '/') {
  const prefix = `${base.replace(/\/?$/, '/')}bron/`;
  const route = (req, res, next) => {
    const pad = (req.url ?? '').split('?')[0];
    if (!pad.startsWith(prefix)) return next();
    const [status, type, tekst] = bronAntwoord(pad.slice(prefix.length).replace(/\/$/, ''));
    res.statusCode = status;
    res.setHeader('Content-Type', type);
    res.setHeader('Cache-Control', 'no-store');
    res.end(tekst);
  };
  return {
    name: 'poc-regels-op-een-plek-bron',
    configureServer(server) { server.middlewares.use(route); },
    configurePreviewServer(server) { server.middlewares.use(route); },
  };
}
