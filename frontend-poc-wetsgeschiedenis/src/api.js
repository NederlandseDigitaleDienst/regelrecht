// De API van de dienst, onder het pad van de poc.
const BASIS = import.meta.env.BASE_URL;

/**
 * Een antwoord als { ok, status, x, s }. Een fout van het portaal (platte
 * tekst) of van de ingress (HTML) is geen JSON; dan zegt x.fout wie antwoordde.
 */
export async function api(pad, opties) {
  const t0 = performance.now();
  let r;
  try {
    r = await fetch(BASIS + pad, opties);
  } catch (e) {
    return { ok: false, status: 0, x: { fout: `Geen verbinding met de dienst (${e.message}).` } };
  }
  const tekst = await r.text();
  const s = Math.round((performance.now() - t0) / 100) / 10;
  try {
    return { ok: r.ok, status: r.status, x: JSON.parse(tekst), s };
  } catch {
    const wie = r.status === 502 ? 'Het portaal kreeg geen antwoord van de dienst'
      : r.status === 504 ? 'De ingress wachtte te lang op de dienst' : 'Een tussenliggende server antwoordde';
    const kort = tekst.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim().slice(0, 160);
    return { ok: false, status: r.status, s, x: { fout: `${wie} (HTTP ${r.status}, na ${s} s)${kort ? ': ' + kort : '.'}` } };
  }
}

export const post = (pad, body) => api(pad, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(body),
});

export const link = (pad) => BASIS + pad;

const MAANDEN = ['jan', 'feb', 'mrt', 'apr', 'mei', 'jun', 'jul', 'aug', 'sep', 'okt', 'nov', 'dec'];
export const datumTekst = (d) => {
  if (!d) return '';
  const [j, m, g] = d.split('-');
  return `${+g} ${MAANDEN[+m - 1]} ${j}`;
};
export const bekendmaking = (x) => (x || '')
  .replace(/^stb-(\d{4})-(\d+)$/, 'Stb. $1, $2')
  .replace(/^stcrt-(\d{4})-(\d+)$/, 'Stcrt. $1, $2');
export const isBwb = (x) => /^BWBR\d{7}$/.test(x);

/** Een dossier of wet openen: de bekijker leest zijn keuze uit het adres. */
export function open(id) {
  try { localStorage.setItem('wg-dossier', id); } catch { /* geen opslag */ }
  location.hash = 'd=' + id;
  location.reload();
}
