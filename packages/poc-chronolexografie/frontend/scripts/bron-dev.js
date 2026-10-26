/**
 * De bronnen op de vite-dev-server (`just poc-chronolexografie`): de route
 * bron/<naam> die achter het poc-portaal het portaal zelf serveert
 * (packages/poc-portal/src/bron.rs), hier uit de lokale map in
 * POC_BRON_<NAAM>. Zonder die variabele antwoordt de route 503, zoals het
 * portaal zonder token, en toont de app "bron niet beschikbaar".
 */
import { existsSync, readdirSync, readFileSync, statSync } from 'fs';
import { join, relative, resolve } from 'path';

/** De env-var met de lokale map van bron `naam`: napp -> POC_BRON_NAPP. */
export const lokaleEnv = (naam) => `POC_BRON_${naam.toUpperCase().replace(/-/g, '_')}`;

/** Elk YAML-bestand onder `dir`, per pad onder `dir`. */
export function bestandenUitMap(dir) {
  const uit = {};
  const loop = (map) => {
    for (const entry of readdirSync(map, { withFileTypes: true })) {
      const pad = join(map, entry.name);
      if (entry.isDirectory()) loop(pad);
      else if (/\.ya?ml$/.test(entry.name)) {
        uit[relative(dir, pad).split('\\').join('/')] = readFileSync(pad, 'utf8');
      }
    }
  };
  loop(dir);
  return uit;
}

/**
 * Het antwoord op bron/<naam>: [status, type, tekst]. Een relatief pad in
 * POC_BRON_<NAAM> geldt vanaf waar `just` of `npm` startte (INIT_CWD), niet
 * vanaf de map van de app, waar npm de dev-server draait.
 */
export function bronAntwoord(naam, env = process.env) {
  const gezet = env[lokaleEnv(naam)];
  const dir = gezet && resolve(env.INIT_CWD ?? process.cwd(), gezet);
  if (!dir) {
    return [503, 'text/plain; charset=utf-8', `bron niet beschikbaar: ${lokaleEnv(naam)} is niet gezet`];
  }
  if (!existsSync(dir) || !statSync(dir).isDirectory()) {
    return [502, 'text/plain; charset=utf-8', `bron kon niet worden opgehaald: ${dir} is geen map`];
  }
  const tekst = JSON.stringify({ naam, versie: 'lokaal', bestanden: bestandenUitMap(dir) });
  return [200, 'application/json', tekst];
}

/** De vite-plugin: bron/<naam> onder de basis van de app. */
export function bronnenDev(base = '/') {
  const prefix = `${base.replace(/\/?$/, '/')}bron/`;
  return {
    name: 'poc-chronolexografie-bronnen',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const pad = (req.url ?? '').split('?')[0];
        if (!pad.startsWith(prefix)) return next();
        const [status, type, tekst] = bronAntwoord(pad.slice(prefix.length).replace(/\/$/, ''));
        res.statusCode = status;
        res.setHeader('Content-Type', type);
        res.setHeader('Cache-Control', 'no-store');
        res.end(tekst);
      });
    },
  };
}
