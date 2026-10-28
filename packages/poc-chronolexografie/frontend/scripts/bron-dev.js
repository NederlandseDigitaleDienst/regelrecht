/**
 * De bronnen op de vite-dev-server (`just poc-chronolexografie`): de route
 * bron/<naam> die achter het poc-portaal het portaal zelf serveert
 * (packages/poc-portal/src/bron.rs), hier uit de lokale map in
 * POC_BRON_<NAAM>, of uit het manifest dat die variabele noemt, zoals het
 * portaal het leest (`Manifest` in bron.rs). Zonder die variabele antwoordt
 * de route 503, zoals het portaal zonder token, en toont de app "bron niet
 * beschikbaar".
 */
import { existsSync, readdirSync, readFileSync, statSync } from 'fs';
import { dirname, join, relative, resolve } from 'path';
import { load as leesYaml } from 'js-yaml';

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

const zonderSlash = (pad) => pad.replace(/^\/+|\/+$/g, '');

/**
 * De bron die het manifest in `bestand` maakt: per map van de bron de map
 * van de repository onder `wortel` (vanaf de map van het manifest). Een
 * bestand hoort bij de langste map die het bevat. Een map zonder YAML is een
 * fout met de naam, zoals in het portaal.
 */
export function bestandenUitManifest(bestand) {
  const { wortel = '', mappen } = leesYaml(readFileSync(bestand, 'utf8'));
  const basis = resolve(dirname(bestand), wortel);
  const paren = Object.entries(mappen).map(([doel, van]) => [zonderSlash(doel), zonderSlash(van)]);
  // Elke map ligt onder de wortel, zoals `Manifest::lees` in het portaal eist.
  for (const pad of paren.flat()) {
    if (!pad || pad.split('/').some((d) => d === '..' || d === '.' || d === '')) {
      throw new Error(`the manifest ${bestand}: '${pad}' is no directory below the root`);
    }
  }
  const alle = {};
  for (const [, van] of paren) {
    const dir = join(basis, van);
    if (!existsSync(dir)) continue;
    for (const [pad, tekst] of Object.entries(bestandenUitMap(dir))) alle[`${van}/${pad}`] = tekst;
  }
  const uit = {};
  for (const [pad, tekst] of Object.entries(alle)) {
    const passend = paren.filter(([, van]) => pad.startsWith(`${van}/`));
    if (!passend.length) continue;
    const [doel, van] = passend.reduce((a, b) => (b[1].length > a[1].length ? b : a));
    uit[`${doel}${pad.slice(van.length)}`] = tekst;
  }
  for (const [doel, van] of paren) {
    if (!Object.keys(alle).some((p) => p.startsWith(`${van}/`))) {
      throw new Error(`the manifest names '${van}' for '${doel}', which holds no YAML file`);
    }
  }
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
  const manifest = /\.ya?ml$/.test(dir);
  if (!existsSync(dir) || statSync(dir).isDirectory() === manifest) {
    const wat = manifest ? 'geen manifest' : 'geen map';
    return [502, 'text/plain; charset=utf-8', `bron kon niet worden opgehaald: ${dir} is ${wat}`];
  }
  let bestanden;
  try {
    bestanden = manifest ? bestandenUitManifest(dir) : bestandenUitMap(dir);
  } catch (e) {
    return [502, 'text/plain; charset=utf-8', `bron kon niet worden opgehaald: ${e.message}`];
  }
  const tekst = JSON.stringify({ naam, versie: 'lokaal', bestanden });
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
