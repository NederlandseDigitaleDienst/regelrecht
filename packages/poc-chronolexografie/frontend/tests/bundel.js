import { readdirSync, readFileSync, statSync } from 'fs';
import { join, relative } from 'path';
import { fileURLToPath } from 'url';
import { CORPUS } from '../src/corpus.js';

const repo = fileURLToPath(new URL('../../../../', import.meta.url));

/**
 * The files of the corpus of the poc, per path in the repository, as
 * scripts/copy-assets.js puts them in public/bundel.json: every file, except
 * what is under a directory `scenarios`.
 */
export function bundel() {
  const alle = {};
  const lees = (dir) => {
    for (const e of readdirSync(dir)) {
      const pad = join(dir, e);
      if (statSync(pad).isDirectory()) {
        if (e !== 'scenarios') lees(pad);
      } else {
        alle[relative(repo, pad).split('\\').join('/')] = readFileSync(pad, 'utf8');
      }
    }
  };
  lees(join(repo, CORPUS));
  return alle;
}
