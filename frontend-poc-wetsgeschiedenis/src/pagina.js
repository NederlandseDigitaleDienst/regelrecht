/* Gemaakt door werkbank/generatoren/bouw-wetsgeschiedenis.py --poc uit
   wetsgeschiedenis-sjabloon.html in het Wpp-dossier; wijzig het daar. */
// ------------------------------------------------------------- welk dossier
// De gegevens staan in deze pagina gebakken (het losse bestand), of naast haar
// als JSON (de poc achter het portaal, waar de CSP geen inline script toelaat).
// BASIS is daar de basis van de bundel; het bouwscript zet hem.
const BASIS = import.meta.env.BASE_URL;
async function gegevens(naam) {
  const blok = document.getElementById(naam);
  if (blok) return JSON.parse(blok.textContent);
  const r = await fetch(BASIS + "gegevens/" + naam + ".json");
  if (!r.ok) {
    document.getElementById("document").textContent = "De gegevens van " + naam + " zijn niet te laden (" + r.status + ").";
    throw new Error("gegevens/" + naam + ".json: " + r.status);
  }
  return r.json();
}
// Een oud dossier (een scan uit Staten-Generaal Digitaal) speelt niet af; zijn
// toelichting staat bij de wet die het maakte.
const DOSSIERS = (await gegevens("dossiers")).filter((d) => d.soort !== "oud dossier");
const KEUZE = (() => {
  const h = (location.hash.match(/d=([0-9A-Za-z]+)/) || [])[1];
  if (h && DOSSIERS.some((d) => d.dossier === h)) return h;
  try { const m = localStorage.getItem("wg-dossier"); if (m && DOSSIERS.some((d) => d.dossier === m)) return m; } catch (e) {}
  return DOSSIERS[0].dossier;
})();
const T = await gegevens(KEUZE);
const $ = (id) => document.getElementById(id);
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const MAANDEN = ["jan", "feb", "mrt", "apr", "mei", "jun", "jul", "aug", "sep", "okt", "nov", "dec"];
const datumTekst = (d) => { if (!d) return ""; const [j, m, g] = d.split("-"); return `${+g} ${MAANDEN[+m - 1]} ${j}`; };
$("dossierkeuze").innerHTML = DOSSIERS.map((d) => `<option value="${esc(d.dossier)}"${d.dossier === KEUZE ? " selected" : ""}>${esc(d.naam)}${d.soort === "keten" ? "" : ` (${d.dossier.replace(/(\d{2})(\d{3})/, "$1 $2")})`}${d.opgehaald ? ", opgehaald" : ""}</option>`).join("");
$("dossierkeuze").addEventListener("change", (e) => {
  try { localStorage.setItem("wg-dossier", e.target.value); } catch (x) {}
  location.hash = "d=" + e.target.value;
  location.reload();
});

// ------------------------------------------------------------- de stukken
const STUK = Object.fromEntries(T.stukken.map((s) => [s.stuk, s]));
const VOORSTEL = T.voorstel;
const nrVan = (stuk) => (STUK[stuk] ? STUK[stuk].ondernummer : stuk.split("-").pop());
// De soort van een stuk bepaalt alleen zijn kleur en zijn label.
function stukSoort(stuk) {
  const x = STUK[stuk] || {}, s = x.soort || "";
  if (stuk === VOORSTEL) return "voorstel";
  // Een wet door de tijd: versies, en de wetten die ze maakten.
  if (/^(Wijzigings(wet|besluit|regeling)|Nieuwe? (wet|besluit|regeling))$/.test(s)) return "wijziging";
  if (s === "Intrekking") return "intrekking";
  if (s === "Aanhangig voorstel") return "aanhangig";
  if (s.startsWith("Nota van wijziging")) return "nota";
  if (s.startsWith("Gewijzigd voorstel")) return "ek";
  if (s.startsWith("Memorie van toelichting")) return "memorie";
  if (s.startsWith("Advies") || s.startsWith("Voorlichting")) return "advies";
  if (s.startsWith("Nota n.a.v.") || s.startsWith("Nota naar aanleiding") || s.startsWith("Memorie van antwoord")) return "antwoord";
  if (/verslag/i.test(s)) return "vraag";
  if (s.startsWith("Amendement")) return "amendement";
  return "brief";
}
const SOORTNAAM = { voorstel: "voorstel van wet", nota: "nota van wijziging", ek: "Eerste Kamer: gewijzigd voorstel", memorie: "memorie van toelichting", advies: "Raad van State", vraag: "verslag (vragen)", antwoord: "nota n.a.v. verslag (antwoorden)", amendement: "amendement", brief: "brief of ander stuk", stemming: "stemming", ijkpunt: "Staatsblad of BWB", wijziging: "wijziging", intrekking: "intrekking", aanhangig: "aanhangig voorstel" };
const NOTAKLEUR = ["--stuk-8", "--stuk-20", "--stuk-ander"];
const NOTAS = T.stukken.filter((s) => s.soort.startsWith("Nota van wijziging")).map((s) => s.stuk);
const streepKlasse = (stuk) => { const i = NOTAS.indexOf(stuk); return i < 0 ? (stuk === VOORSTEL ? "" : "sander") : ["s8", "s20", "sander"][Math.min(i, 2)]; };
function kleur(stuk) {
  const s = stukSoort(stuk);
  if (s === "nota") return `var(${NOTAKLEUR[Math.min(NOTAS.indexOf(stuk), 2)]})`;
  if (s === "amendement" && (STUK[stuk] || {}).speelt_mee) return "var(--stuk-ander)";
  return `var(--k-${s})`;
}
const STATUS = { ok: ["afgespeeld", "success"], aanname: ["met aanname", "warning"], niet: ["niet afgespeeld", "critical"] };
function refTekst(r) {
  const x = T.refs[r];
  if (x.stuk === VOORSTEL) return `nr. ${nrVan(VOORSTEL)}, voorstel van wet`;
  return `nr. ${nrVan(x.stuk)}${x.onderdeel ? ", onderdeel " + x.onderdeel : ""}${x.sub ? " onder " + x.sub : ""}`;
}

// ------------------------------------------------------------- de stand per handeling
// Stand 0 is het voorstel; stand i (1..N) is de tekst na handeling i.
const N = T.stappen.length;
const STAPPEN = T.stappen.map((s, i) => Object.assign({ i: i + 1 }, s));
const IDENT = T.identiteit || {};
const idVan = (k) => IDENT[k] || k;
const CHECKPUNT = new Map();
const STANDCACHE = new Map();
function stand(i) {
  if (STANDCACHE.has(i)) return STANDCACHE.get(i);
  let start = Math.floor(i / 25) * 25, b, v;
  while (start > 0 && !CHECKPUNT.has(start)) start -= 25;
  if (start === 0) { b = Object.assign({}, T.begin.blokken); v = T.begin.volgorde; }
  else { const c = CHECKPUNT.get(start); b = Object.assign({}, c.b); v = c.v; }
  for (let k = start; k < i; k++) {
    const s = T.stappen[k];
    Object.assign(b, s.wijzig);
    for (const w of s.weg) delete b[w];
    if (s.volgorde) v = s.volgorde;
    if ((k + 1) % 25 === 0 && !CHECKPUNT.has(k + 1)) CHECKPUNT.set(k + 1, { b: Object.assign({}, b), v });
  }
  const uit = { b, v };
  if (STANDCACHE.size > 40) STANDCACHE.delete(STANDCACHE.keys().next().value);
  STANDCACHE.set(i, uit);
  return uit;
}
// Een artikel heet in de pagina naar zijn identiteit: de sleutel waarmee het
// begon. Zo blijft het hetzelfde artikel als het een ander nummer krijgt.
function artikelIn(S, id) {
  for (const k of S.v) { const b = S.b[k]; if (b.soort === "artikel" && idVan(k) === id) return [k, b]; }
  return [null, null];
}
function idVanNr(S, nr) {
  for (const k of S.v) { const b = S.b[k]; if (b.soort === "artikel" && b.nr === nr) return idVan(k); }
  return null;
}
function nrVanId(id) {
  for (const i of [Z.stap, N, 0]) { const [, b] = artikelIn(stand(i), id); if (b) return b.nr; }
  return id;
}

// ------------------------------------------------------------- de tijd
// De momenten komen uit de tijdlijn: elk stuk op zijn datum, een stemming,
// een Staatsblad. Een moment met handelingen speelt die daarna af.
const TIJD = [], POSSTUK = {}, POSSTAP = [];
{
  let k = 0;
  T.momenten.forEach((m, mi) => {
    if (m.soort === "stuk") POSSTUK[m.stuk] = TIJD.length;
    TIJD.push({ soort: m.soort, stuk: m.stuk || null, moment: m, stap: k });
    if (m.stappen) for (let i = m.stappen[0]; i <= m.stappen[1]; i++) { k = i; POSSTAP[i] = TIJD.length; TIJD.push({ soort: "stap", stuk: STAPPEN[i - 1].stuk, moment: m, stap: i }); }
  });
}
const P = TIJD.length - 1;
const MOMENTEN = TIJD.map((t, p) => [t, p]).filter(([t]) => t.soort !== "stap");
const datumVan = (p) => TIJD[p].moment.datum;
const laatstePos = (p) => { while (p + 1 <= P && TIJD[p + 1].soort === "stap") p++; return p; };
function momentKleur(t) {
  if (t.soort === "stemming") return "var(--k-stemming)";
  if (t.soort === "ijkpunt") return "var(--k-ijkpunt)";
  return kleur(t.stuk);
}

// Welke artikelen elke handeling raakt: wat zij veranderde, of, als zij niet
// afspeelde, het artikel dat haar opdracht noemt.
const STAPART = STAPPEN.map((s) => {
  const ids = new Set(Object.entries(s.wijzig).filter(([, b]) => b.soort === "artikel").map(([k]) => idVan(k)));
  if (!ids.size) { const m = (s.opdracht || "").match(/artikel ((?:[A-Z][a-z]? )?\d+[a-z]*(?:[.:]\d+[a-z]*)*|[IVXL]+)/i); if (m) { const id = idVanNr(stand(s.i - 1), m[1]); if (id) ids.add(id); } }
  return [...ids];
});
const RAAK = new Map();
STAPART.forEach((ids, k) => ids.forEach((id) => { if (!RAAK.has(id)) RAAK.set(id, []); RAAK.get(id).push(k + 1); }));
const REFSTAP = new Map();
STAPPEN.forEach((s) => T.refs.forEach((x, r) => {
  if (x.stuk === s.stuk && x.onderdeel === s.onderdeel && (x.sub ?? null) === (s.sub ?? null) && !REFSTAP.has(r)) REFSTAP.set(r, s.i);
}));

// De toelichting per artikel (naar identiteit) en per artikel van de doelwet.
const BROKKEN = T.commentaar.brokken;
const PERART = new Map(), PERDOEL = new Map(), PERSTUK = new Map();
for (const b of BROKKEN) {
  for (const id of b.ids || []) if (id) { if (!PERART.has(id)) PERART.set(id, []); PERART.get(id).push(b); }
  for (const nr of b.doelartikelen || []) { if (!PERDOEL.has(nr)) PERDOEL.set(nr, []); PERDOEL.get(nr).push(b); }
  if (!PERSTUK.has(b.stuk)) PERSTUK.set(b.stuk, []);
  PERSTUK.get(b.stuk).push(b);
}
const bekend = (b) => (POSSTUK[b.stuk] ?? 0) <= Z.pos;
const EIND = T.ijkpunten.length ? T.ijkpunten[T.ijkpunten.length - 1] : null;

// ------------------------------------------------------------- woorddiff
function woorden(t) { return t.match(/\s+|[^\s]+/g) || []; }
function lcs(a, b, gelijk) {
  const n = a.length, m = b.length;
  if (n * m > 4e6) return null;
  const d = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--)
    d[i][j] = gelijk(a[i], b[j]) ? d[i + 1][j + 1] + 1 : Math.max(d[i + 1][j], d[i][j + 1]);
  const uit = []; let i = 0, j = 0;
  while (i < n && j < m) {
    if (gelijk(a[i], b[j])) { uit.push(["=", a[i], b[j]]); i++; j++; }
    else if (d[i + 1][j] >= d[i][j + 1]) uit.push(["-", a[i++]]);
    else uit.push(["+", b[j++]]);
  }
  while (i < n) uit.push(["-", a[i++]]);
  while (j < m) uit.push(["+", b[j++]]);
  return uit;
}
function woorddiff(oud, nieuw) {
  const ops = lcs(woorden(oud), woorden(nieuw), (x, y) => x === y);
  if (!ops) return `<del>${esc(oud)}</del> <ins>${esc(nieuw)}</ins>`;
  let h = "";
  for (const [o, x] of ops) h += o === "=" ? esc(x) : o === "-" ? (x.trim() ? `<del>${esc(x)}</del>` : "") : `<ins>${esc(x)}</ins>`;
  return h.replace(/<\/del><del>/g, "").replace(/<\/ins><ins>/g, "");
}
function regeldiff(oud, nieuw) {
  const ops = lcs(oud, nieuw, (x, y) => x[0] === y[0]) || [...oud.map((x) => ["-", x]), ...nieuw.map((x) => ["+", x])];
  const uit = [];
  for (let k = 0; k < ops.length; k++) {
    const [o, x, y] = ops[k];
    if (o === "=") { uit.push({ soort: "gelijk", regel: y }); continue; }
    const weg = [], bij = [];
    while (k < ops.length && ops[k][0] !== "=") { (ops[k][0] === "-" ? weg : bij).push(ops[k][1]); k++; }
    k--;
    const paar = Math.min(weg.length, bij.length);
    for (let p = 0; p < paar; p++) uit.push({ soort: "gewijzigd", regel: bij[p], html: woorddiff(weg[p][0], bij[p][0]) });
    for (let p = paar; p < bij.length; p++) uit.push({ soort: "nieuw", regel: bij[p] });
    for (let p = paar; p < weg.length; p++) uit.push({ soort: "vervallen", regel: weg[p] });
  }
  return uit;
}
// Zoals de vergelijking in afspelen.py leest: aanhalingstekens gelijk, witruimte enkel.
const norm = (t) => t.replace(/[“„]/g, "«").replace(/”/g, "»").replace(/[‘’']/g, "'").split(/\s+/).filter(Boolean).join(" ");
const genormd = (regels) => regels.map((r) => [norm(r[0]), r[1], r[2], r[3]]);
const alsRegels = (lijnen) => lijnen.map((t, i) => [t, [], 0, i ? "al" : "kop"]);
function miniDiff(oud, nieuw) {
  const ds = regeldiff(oud, nieuw);
  let h = "", vorige = null;
  ds.forEach((d) => {
    if (d.soort === "gelijk") { vorige = d; return; }
    if (vorige) h += `<div class="diffregel zacht">${esc(vorige.regel[0].slice(0, 90))}${vorige.regel[0].length > 90 ? "…" : ""}</div>`;
    vorige = null;
    const cls = d.soort === "nieuw" ? " nieuw" : d.soort === "vervallen" ? " vervallen" : "";
    h += `<div class="diffregel${cls}">${d.html ?? esc(d.regel[0])}</div>`;
  });
  return h || `<div class="klein">Geen verschil in de tekst.</div>`;
}

// ------------------------------------------------------------- toestand van de pagina
const Z = { pos: P, modus: "tekst", regel: null, art: null, volg: true, toekomst: false, zoek: "", alleenGewijzigd: false, speelt: null, van: "nu", tot: EIND ? "ijk:" + (T.ijkpunten.length - 1) : "s:0", dw: 0, dv: -1 };
Object.defineProperty(Z, "stap", { get() { return TIJD[Z.pos].stap; } });
try { const m = localStorage.getItem("wg-modus"); if (m) Z.modus = m; } catch (e) {}
if (Z.modus === "doorwerking" && !T.doorwerking.length) Z.modus = "tekst";
{
  const h = location.hash;
  const p = parseInt((h.match(/t=(\d+)/) || [])[1], 10); if (!isNaN(p)) Z.pos = Math.max(0, Math.min(P, p));
  const a = (h.match(/art=([0-9A-Za-z]+)/) || [])[1]; if (a) { Z.art = a; Z.volg = false; }
}
const huidig = () => TIJD[Z.pos];
const huidigeStap = () => huidig().soort === "stap" ? STAPPEN[huidig().stap - 1] : null;

function uitslagTekst(s) {
  if (!s) return "";
  if (s.uitslag) return `${s.uitslag.uitslag} op ${datumTekst(s.uitslag.datum)}`;
  return s.niet_mee || "";
}
function momentLabel(p) {
  const t = TIJD[p], m = t.moment;
  const datum = `<span class="datum">${datumTekst(m.datum)}</span>`;
  if (t.soort === "stemming") {
    const n = m.stukken.length;
    return `${datum}<span>Stemming in de Tweede Kamer</span> <span class="zacht">· ${n ? `${n} ${n === 1 ? "amendement" : "amendementen"} aangenomen, spelen af` : "geen aangenomen amendementen"}${m.voorstel ? ` · het voorstel ${m.voorstel}` : ""}</span>`;
  }
  if (t.soort === "ijkpunt") return `${datum}<span>${esc(m.naam)}</span> <span class="zacht">· ijkpunt</span>`;
  if (t.soort === "stuk") {
    const st = STUK[t.stuk] || {};
    const n = (PERSTUK.get(t.stuk) || []).length;
    const extra = m.stappen ? `${m.stappen[1] - m.stappen[0] + 1} handelingen volgen` : st.soort && st.soort.startsWith("Amendement") ? uitslagTekst(st) : n ? `${n} ${n === 1 ? "brok" : "brokken"} toelichting` : "geen brokken aan een artikel";
    return `${datum}<span>nr. ${esc(nrVan(t.stuk))} · ${esc(st.soort || "")}${st.kamer === "Eerste Kamer" ? " (Eerste Kamer)" : ""}</span> <span class="zacht">· ${esc(extra)}</span>`;
  }
  const s = STAPPEN[t.stap - 1];
  return `${datum}<span>Handeling ${s.i} van ${N}</span> <span class="zacht">· nr. ${esc(nrVan(s.stuk))}, onderdeel ${esc(s.onderdeel)}${s.sub ? " onder " + s.sub : ""}${s.ronde === 2 ? ", tweede ronde" : ""}${s.veeg ? ", veegactie" : ""} · ${STATUS[s.status][0]}</span>`;
}

// ------------------------------------------------------------- kalender en strook
function bouwKalender() {
  const data = MOMENTEN.map(([t]) => t.moment.datum).filter(Boolean).sort();
  const d0 = Date.parse(data[0]), d1 = Date.parse(data[data.length - 1]);
  const x = (d) => 1.5 + 97 * (Date.parse(d) - d0) / Math.max(1, d1 - d0);
  let h = `<div class="as"></div><div class="verstreken" id="verstreken"></div>`;
  const maanden = Math.max(1, (d1 - d0) / 2.6e9);
  const stap = Math.max(1, Math.ceil(maanden / 14));
  const begin = new Date(d0); begin.setDate(1); begin.setMonth(begin.getMonth() + 1);
  for (let m = new Date(begin), i = 0; m.getTime() <= d1; m.setMonth(m.getMonth() + 1), i++) {
    if (i % stap) continue;
    h += `<span class="maand" style="left:${x(m.toISOString().slice(0, 10))}%">${MAANDEN[m.getMonth()]} ${String(m.getFullYear()).slice(2)}</span>`;
  }
  const opDatum = new Map();
  for (const [t, p] of MOMENTEN) {
    if (!t.moment.datum) continue;
    const n = opDatum.get(t.moment.datum) || 0; opDatum.set(t.moment.datum, n + 1);
    const st = STUK[t.stuk] || {};
    const titel = t.soort === "stemming" ? `Stemming, ${datumTekst(t.moment.datum)}` : t.soort === "ijkpunt" ? `${t.moment.naam}, ${datumTekst(t.moment.datum)}` : `nr. ${st.ondernummer}, ${st.soort}, ${datumTekst(st.datum)}: ${st.titel}`;
    const kl = t.soort === "stuk" ? (st.kamer === "Eerste Kamer" ? "ek" : "") : t.soort;
    h += `<button class="stip ${kl}" data-pos="${p}" style="left:${x(t.moment.datum)}%;top:${30 - Math.min(n, 3) * 9}px;background:${momentKleur(t)}" title="${esc(titel)}"></button>`;
  }
  h += `<div class="kalnaald" id="kalnaald"></div>`;
  $("kalender").innerHTML = h;
  $("kalender").addEventListener("click", (e) => { const b = e.target.closest(".stip"); if (b) { stop(); ga(+b.dataset.pos); } });
  bouwKalender.x = x;
}
function bouwStrook() {
  const el = $("strook");
  let h = "";
  for (const [t, p] of MOMENTEN) if (t.moment.stappen) {
    const tot = laatstePos(p);
    const naam = t.soort === "stemming" ? "stemming" : `nr. ${nrVan(t.stuk)} ${(STUK[t.stuk] || {}).soort.toLowerCase()}`;
    h += `<div class="deel" style="left:${(p / (P + 1)) * 100}%;width:${((tot - p + 1) / (P + 1)) * 100}%">${esc(naam)}, ${datumTekst(t.moment.datum)}</div>`;
  }
  h += `<div class="ticks" id="ticks">`;
  TIJD.forEach((t) => {
    if (t.soort !== "stap") h += `<span class="tick moment" style="background:${momentKleur(t)}"></span>`;
    else { const s = STAPPEN[t.stap - 1]; h += `<span class="tick ${s.status}" title="Handeling ${s.i}: nr. ${esc(nrVan(s.stuk))} ${esc(s.onderdeel)}${s.sub ? " onder " + s.sub : ""}, ${STATUS[s.status][0]}"></span>`; }
  });
  h += `</div><div class="naald" id="naald"></div>`;
  el.innerHTML = h;
  const ticks = [...$("ticks").children];
  const naar = (e) => {
    let beste = 0, afstand = Infinity;
    ticks.forEach((t, p) => { const r = t.getBoundingClientRect(); const d = Math.abs(r.left + r.width / 2 - e.clientX); if (d < afstand) { afstand = d; beste = p; } });
    ga(beste);
  };
  let sleept = false;
  el.addEventListener("pointerdown", (e) => { sleept = true; el.setPointerCapture(e.pointerId); stop(); naar(e); });
  el.addEventListener("pointermove", (e) => { if (sleept) naar(e); });
  el.addEventListener("pointerup", () => { sleept = false; });
  const soorten = [...new Set(T.stukken.map((s) => stukSoort(s.stuk)))].filter((s) => s !== "nota");
  if (T.momenten.some((m) => m.soort === "stemming")) soorten.push("stemming");
  if (T.momenten.some((m) => m.soort === "ijkpunt")) soorten.push("ijkpunt");
  $("legenda").innerHTML = soorten.map((s) => `<span><i class="rond" style="background:var(--k-${s})"></i>${SOORTNAAM[s]}</span>`).join("")
    + NOTAS.slice(0, 3).map((s, i) => `<span><i style="background:var(${NOTAKLEUR[i]})"></i>nr. ${esc(nrVan(s))}</span>`).join("")
    + `<span><i style="background:var(--st-ok)"></i>afgespeeld</span><span><i style="background:var(--st-aanname)"></i>met aanname</span><span><i style="background:var(--st-niet)"></i>niet afgespeeld</span>`
    + `<span><kbd>←</kbd> <kbd>→</kbd> stap, <kbd>spatie</kbd> afspelen</span>`;
}
function tekenNaald() {
  const ticks = $("ticks").children;
  const t = ticks[Z.pos], r = t.getBoundingClientRect(), o = $("strook").getBoundingClientRect();
  $("naald").style.left = `${r.left - o.left + r.width / 2}px`;
  for (let k = 0; k < ticks.length; k++) ticks[k].classList.toggle("voorbij", k <= Z.pos);
  $("strook").setAttribute("aria-valuenow", Z.pos);
  $("strook").setAttribute("aria-valuetext", momentLabel(Z.pos).replace(/<[^>]+>/g, ""));
  $("stapnaam").innerHTML = momentLabel(Z.pos);
  const d = datumVan(Z.pos);
  if (d) { const x = bouwKalender.x(d); $("kalnaald").style.left = x + "%"; $("verstreken").style.width = x + "%"; }
  let mom = Z.pos; while (mom > 0 && TIJD[mom].soort === "stap") mom--;
  document.querySelectorAll("#kalender .stip").forEach((b) => {
    b.classList.toggle("geweest", +b.dataset.pos <= Z.pos);
    b.classList.toggle("nu", +b.dataset.pos === mom);
  });
}

// ------------------------------------------------------------- dossier en artikellijst
function bouwStukken() {
  let mom = Z.pos; while (mom > 0 && TIJD[mom].soort === "stap") mom--;
  $("stukken").innerHTML = MOMENTEN.map(([t, p]) => {
    if (t.soort !== "stuk") {
      const naam = t.soort === "stemming" ? "Stemming Tweede Kamer" : t.moment.naam;
      return `<li class="${p > Z.pos ? "later" : ""}${p === mom ? " actief" : ""}" data-pos="${p}"><span class="punt" style="background:${momentKleur(t)}"></span><span class="nr">·</span><span><span class="soort">${esc(naam)}</span><span class="datum">${datumTekst(t.moment.datum)}${t.moment.stappen ? " · speelt af" : ""}</span></span></li>`;
    }
    const s = STUK[t.stuk];
    const n = (PERSTUK.get(s.stuk) || []).length;
    return `<li class="${p > Z.pos ? "later" : ""}${p === mom ? " actief" : ""}" data-pos="${p}" title="${esc(s.titel)}">
      <span class="punt" style="background:${kleur(s.stuk)}"></span>
      <span class="nr">${esc(s.ondernummer)}</span>
      <span><span class="soort">${esc(s.soort)}</span><span class="datum">${datumTekst(s.datum)}${s.kamer === "Eerste Kamer" ? " · Eerste Kamer" : ""}${n ? ` · ${n} brokken` : ""}${t.moment.stappen ? " · speelt af" : ""}${s.soort.startsWith("Amendement") ? " · " + esc(uitslagTekst(s)) : ""}</span></span></li>`;
  }).join("");
}
$("stukken").addEventListener("click", (e) => {
  const li = e.target.closest("li[data-pos]"); if (!li) return;
  stop(); ga(+li.dataset.pos);
});

function bouwArtikellijst(S) {
  const s = huidigeStap();
  const nu = new Set(s ? STAPART[s.i - 1] : (PERSTUK.get(huidig().stuk) || []).flatMap((b) => b.ids || []));
  const zoek = Z.zoek.toLowerCase();
  let h = "";
  for (const k of S.v) {
    const b = S.b[k];
    if (b.soort !== "artikel") {
      if (!zoek && !Z.alleenGewijzigd && (b.soort === "hoofdstuk" || b.soort === "deel")) h += `<li class="kopregel">${esc(b.regels[0][0])}</li>`;
      continue;
    }
    const id = idVan(k);
    const keer = (RAAK.get(id) || []).filter((i) => i <= Z.stap).length;
    if (Z.alleenGewijzigd && !keer) continue;
    if (zoek && !(`artikel ${b.nr} ` + b.regels.map((r) => r[0]).join(" ")).toLowerCase().includes(zoek)) continue;
    const titel = b.regels[0][0].replace(/^Artikel \S+\.?\s*/, "");
    h += `<li data-art="${esc(id)}" class="${nu.has(id) ? "nu" : ""}${Z.art === id ? " focus" : ""}"><span class="anr">Art. ${esc(b.nr)}</span><span class="atitel">${esc(titel)}</span>${keer ? `<span class="teller" title="${keer} keer gewijzigd tot hier">${keer}×</span>` : ""}</li>`;
  }
  $("artlijst").innerHTML = h || `<li class="kopregel">Niets gevonden</li>`;
}
$("artlijst").addEventListener("click", (e) => {
  const li = e.target.closest("li[data-art]"); if (!li) return;
  kiesArtikel(li.dataset.art, true);
});

// ------------------------------------------------------------- het document
function regelHtml(r, sleutel, n, extra, html) {
  const laatste = r[1][r[1].length - 1];
  const stuk = laatste !== undefined && T.refs[laatste] ? T.refs[laatste].stuk : VOORSTEL;
  const kl = ["regel", r[3] === "kop" ? "kop" : "", r[3] === "begrip" ? "begrip" : "", r[2] ? "diepte" + Math.min(3, r[2]) : "", extra || ""];
  if (Z.regel && Z.regel.sleutel === sleutel && Z.regel.n === n) kl.push("gekozen");
  return `<div class="${kl.join(" ")}" data-sleutel="${sleutel}" data-n="${n}" title="${esc(r[1].map(refTekst).join(" → "))}"><span class="streep ${streepKlasse(stuk)}"></span><span class="inhoud">${html ?? esc(r[0])}</span></div>`;
}
function toelKnop(id) {
  const alle = PERART.get(id) || [];
  const nu = alle.filter(bekend);
  const vers = alle.some((b) => POSSTUK[b.stuk] === Z.pos);
  if (!alle.length) return "";
  return `<button class="toelknop${Z.art === id ? " aan" : ""}" data-art="${esc(id)}" title="De toelichting bij dit artikel, tot dit moment">${vers ? `<span class="nieuw"></span>` : ""}toelichting ${nu.length}${nu.length < alle.length ? `<span class="zacht">/${alle.length}</span>` : ""}</button>`;
}
function blokHtml(sleutel, b, vorig, geraakt, besproken) {
  if (b.soort !== "artikel") {
    return `<div class="structuurkop ${b.soort}${geraakt ? " geraakt" : ""}" data-sleutel="${sleutel}">${geraakt && vorig ? woorddiff(vorig.regels[0][0], b.regels[0][0]) : esc(b.regels[0][0])}</div>`;
  }
  const id = idVan(sleutel);
  const kl = ["artikel", geraakt ? "geraakt" : "", besproken ? "besproken" : "", Z.art === id ? "focus" : ""].join(" ");
  let h = `<section class="${kl}" data-sleutel="${sleutel}" data-art="${esc(id)}" id="art-${esc(id)}"><div class="knoppen">${toelKnop(id)}</div>`;
  if (geraakt && vorig) {
    regeldiff(vorig.regels, b.regels).forEach((d) => {
      const idx = b.regels.indexOf(d.regel);
      h += regelHtml(d.regel, sleutel, idx < 0 ? -1 : idx, d.soort === "gelijk" || d.soort === "gewijzigd" ? "" : d.soort, d.html);
    });
  } else if (geraakt) {
    b.regels.forEach((r, n) => { h += regelHtml(r, sleutel, n, "nieuw"); });
  } else {
    b.regels.forEach((r, n) => { h += regelHtml(r, sleutel, n); });
  }
  return h + "</section>";
}
function tekenTekst() {
  const S = stand(Z.stap);
  const s = huidigeStap();
  const V = s ? stand(Z.stap - 1) : null;
  const besproken = new Set(s ? [] : (PERSTUK.get(huidig().stuk) || []).flatMap((b) => b.ids || []));
  let h = "";
  for (const k of S.v) {
    const b = S.b[k];
    const geraakt = s && k in s.wijzig;
    h += blokHtml(k, b, geraakt ? V.b[k] : null, geraakt, b.soort === "artikel" && besproken.has(idVan(k)));
  }
  if (s) for (const k of s.weg) {
    const b = V.b[k];
    if (b && b.soort === "artikel") h += `<section class="artikel weg" data-sleutel="${k}"><div class="meta">Artikel ${esc(b.nr)} vervalt in deze handeling</div>${b.regels.map((r) => `<div class="regel vervallen"><span class="streep"></span><span class="inhoud">${esc(r[0])}</span></div>`).join("")}</section>`;
  }
  $("document").innerHTML = h;
  bouwArtikellijst(S);
}
function scrollNaar(id, zacht) {
  const el = id && document.getElementById("art-" + id);
  if (el) el.scrollIntoView({ block: "start", inline: "nearest", behavior: zacht ? "smooth" : "auto" });
}

// ---- twee momenten naast elkaar, of een moment naast een ijkpunt
function momenten() {
  const uit = [["s:0", `Voorstel van wet (nr. ${nrVan(VOORSTEL)}, ${datumTekst((STUK[VOORSTEL] || {}).datum)})`]];
  for (const [t, p] of MOMENTEN) if (t.moment.stappen) {
    const naam = t.soort === "stemming" ? "Na de stemming" : `Na nr. ${nrVan(t.stuk)}, ${(STUK[t.stuk] || {}).soort.toLowerCase()}`;
    uit.push([`s:${t.moment.stappen[1]}`, `${naam} (${datumTekst(t.moment.datum)})`]);
  }
  uit.push(["nu", "Het moment in de speler"]);
  T.ijkpunten.forEach((x, i) => uit.push([`ijk:${i}`, `Ijkpunt: ${x.naam}`]));
  return uit;
}
function standVan(w) {
  if (w.startsWith("ijk:")) {
    const x = T.ijkpunten[+w.slice(4)];
    const v = [], b = {};
    for (const [nr, regels] of Object.entries(x.artikelen)) { const k = "e" + nr; v.push(k); b[k] = { soort: "artikel", nr, regels: alsRegels(regels) }; }
    return { v, b, ijk: x };
  }
  return stand(w === "nu" ? Z.stap : +w.slice(2));
}
function tekenVerschil() {
  const opties = (w) => momenten().map(([v, t]) => `<option value="${v}"${v === w ? " selected" : ""}>${esc(t)}</option>`).join("");
  const A = standVan(Z.van), B = standVan(Z.tot);
  const metIjk = A.ijk || B.ijk;
  let h = `<div class="vergelijk">Van <select id="v-van">${opties(Z.van)}</select> naar <select id="v-tot">${opties(Z.tot)}</select> <span class="zacht" id="v-tel"></span></div>`;
  for (const x of [A.ijk, B.ijk]) if (x) h += `<div class="groepkop"><p><b>${esc(x.naam)}</b>: ${esc(x.uitleg)}. Bij deze vergelijking tellen aanhalingstekens en witruimte niet mee.</p></div>`;
  let tel = 0;
  const per = (S) => new Map(S.v.filter((k) => S.b[k].soort === "artikel").map((k) => [S.b[k].nr, [k, S.b[k]]]));
  const a = per(A), b = per(B), gezien = new Set();
  const regels = (x) => metIjk ? genormd(x.regels) : x.regels;
  const knop = (nr) => { const id = idVanNr(stand(Z.stap), nr) || idVanNr(stand(N), nr); return id ? ` · <button class="naarart" data-art="${esc(id)}">naar artikel ${esc(nr)}</button>` : ""; };
  for (const [nr, [, nb]] of b) {
    gezien.add(nr);
    const oud = a.get(nr);
    const ro = oud ? regels(oud[1]) : null, rn = regels(nb);
    if (ro && JSON.stringify(ro.map((r) => r[0])) === JSON.stringify(rn.map((r) => r[0]))) continue;
    tel++;
    h += `<section class="artikel"><div class="meta">${oud ? "verschilt" : "alleen rechts"}${knop(nr)}</div>`;
    const ds = ro ? regeldiff(ro, rn) : rn.map((r) => ({ soort: "nieuw", regel: r }));
    for (const d of ds) { const cls = d.soort === "nieuw" ? " nieuw" : d.soort === "vervallen" ? " vervallen" : ""; h += `<div class="diffregel${cls}${d.regel[3] === "kop" ? " kop" : ""}">${d.html ?? esc(d.regel[0])}</div>`; }
    h += `</section>`;
  }
  for (const [nr, [, oud]] of a) if (!gezien.has(nr)) {
    tel++;
    h += `<section class="artikel weg"><div class="meta">alleen links (artikel ${esc(nr)})</div>${oud.regels.map((r) => `<div class="diffregel vervallen">${esc(r[0])}</div>`).join("")}</section>`;
  }
  if (!tel) h += `<p class="leeg">Tussen deze twee geen verschil.</p>`;
  $("document").innerHTML = h;
  $("v-tel").textContent = tel ? `${tel} artikelen verschillen` : "";
  $("v-van").onchange = (e) => { Z.van = e.target.value; teken(); };
  $("v-tot").onchange = (e) => { Z.tot = e.target.value; teken(); };
  bouwArtikellijst(stand(Z.stap));
}

// ---- de doorwerking van een wijzigingswet in de wet die zij wijzigt
function tekenDoorwerking() {
  const D = T.doorwerking[Z.dw] || T.doorwerking[0];
  if (!D) { $("document").innerHTML = `<p class="leeg">Dit dossier wijzigt geen wet die hier als bron staat.</p>`; return; }
  const vi = Z.dv < 0 || Z.dv >= D.versies.length ? D.versies.length - 1 : Z.dv;
  const V = D.versies[vi];
  let h = `<div class="doorwerkkop">Wet <select id="dw-wet">${T.doorwerking.map((d, i) => `<option value="${i}"${i === Z.dw ? " selected" : ""}>${esc(d.titel)}</option>`).join("")}</select>
    volgens <select id="dw-versie">${D.versies.map((v, i) => `<option value="${i}"${i === vi ? " selected" : ""}>${esc(v.versie)}${v.datum ? ", " + datumTekst(v.datum) : ""}</option>`).join("")}</select>
    <span class="zacht">${V.tel.afgespeeld} afgespeeld, ${V.tel.aanname} met aanname, ${V.tel.niet} niet</span></div>
    ${V.ijk ? `<div class="groepkop"><p><b>Gemeten tegen ${esc(V.ijk.naam)}</b>: ${V.ijk.gelijk} artikelen gelijk, ${V.ijk.verschillend.length} verschillend${V.ijk.verschillend.length ? ` (${V.ijk.verschillend.slice(0, 20).map(esc).join(", ")})` : ""}.</p></div>` : ""}
    <div class="groepkop"><h2>${esc(D.titel)} zoals ${esc(V.versie.toLowerCase())} haar wijzigt</h2><p>Vertrekpunt: ${esc(D.basis)}. Elk onderdeel van het wijzigingsartikel (${esc(V.artikel || "")}) is als opdracht op die tekst afgespeeld; links wat er stond, de wijzigingen gemarkeerd. Een onderdeel dat niet te lezen is, wordt gemeld en niet geraden.</p></div>`;
  const perArt = new Map();
  for (const x of V.handelingen) for (const nr of x.artikelen) { if (!perArt.has(nr)) perArt.set(nr, []); perArt.get(nr).push(x); }
  for (const [nr, x] of Object.entries(V.artikelen)) {
    const voor = alsRegels(x.voor), na = alsRegels(x.na);
    const hs = perArt.get(nr) || [];
    const toel = (PERDOEL.get(nr) || []).filter((b) => b.rol === "artikelsgewijs");
    const ijkTag = V.ijk ? (V.ijk.verschillend.includes(nr) ? ` · <span style="color:var(--st-aanname)">wijkt af van ${esc(V.ijk.naam)}</span>` : ` · <span style="color:var(--st-ok)">gelijk aan ${esc(V.ijk.naam)}</span>`) : "";
    h += `<section class="artikel"><div class="meta">Artikel ${esc(nr)} van de ${esc(D.titel)}${!x.voor.length ? " · nieuw" : !x.na.length ? " · vervalt" : ""}${ijkTag}</div>`;
    h += voor.length ? regeldiff(voor, na).map((d) => `<div class="diffregel${d.soort === "nieuw" ? " nieuw" : d.soort === "vervallen" ? " vervallen" : ""}${d.regel[3] === "kop" ? " kop" : ""}">${d.html ?? esc(d.regel[0])}</div>`).join("") : na.map((r) => `<div class="diffregel nieuw">${esc(r[0])}</div>`).join("");
    if (hs.length) h += `<ul class="handelingen">${hs.map((x) => `<li><span class="st ${x.status}">${esc(x.onderdeel || "")}${x.sub ? " " + esc(x.sub) : ""} ${STATUS[x.status][0]}</span><span>${esc(x.opdracht.slice(0, 260))}${x.reden ? `<br><span class="klein zacht">${esc(x.reden)}</span>` : ""}</span></li>`).join("")}</ul>`;
    if (toel.length) h += `<details class="meer"><summary>Toelichting (${toel.length})</summary>${toel.map((b) => `<p><b>${esc(b.titel || "")}</b></p>` + b.tekst.slice(0, 6).map((p) => `<p>${esc(p[0])}</p>`).join("")).join("")}</details>`;
    h += `</section>`;
  }
  const nietHier = V.handelingen.filter((x) => x.status === "niet" && !x.artikelen.some((nr) => V.artikelen[nr]));
  if (nietHier.length) h += `<section class="artikel"><div class="meta">Niet afgespeeld, en daarom in geen artikel hierboven</div><ul class="handelingen">${nietHier.map((x) => `<li><span class="st niet">${esc(x.onderdeel || "")}${x.sub ? " " + esc(x.sub) : ""}</span><span>${esc(x.opdracht.slice(0, 260))}<br><span class="klein zacht">${esc(x.reden)}</span></span></li>`).join("")}</ul></section>`;
  $("document").innerHTML = h;
  $("dw-wet").onchange = (e) => { Z.dw = +e.target.value; Z.dv = -1; teken(); };
  $("dw-versie").onchange = (e) => { Z.dv = +e.target.value; teken(); };
  bouwArtikellijst(stand(Z.stap));
}

// ---- controle tegen het eindpunt, ingedeeld naar oorzaak
function tekenControle() {
  if (!EIND) { $("document").innerHTML = `<p class="leeg">Er is geen ijkpunt om de afspeling aan te meten.</p>`; bouwArtikellijst(stand(Z.stap)); return; }
  const per = {};
  for (const [nr, x] of Object.entries(T.bron)) (per[x.groep] = per[x.groep] || []).push([nr, x]);
  const totaal = Object.keys(T.bron).length;
  let h = `<div class="groepkop"><h2>De afgespeelde tekst naast ${esc(EIND.naam)}</h2>
    <p>${esc(EIND.uitleg)}. ${T.gelijk} artikelen zijn woord voor woord gelijk, ${totaal} verschillen. Groen is wat het ijkpunt heeft en het afspelen niet, rood wat het afspelen oplevert en het ijkpunt niet. De indeling is een wegwijzer, geen oordeel: elk verschil vraagt een blik van een mens.</p>
    ${T.ijkpunten.length > 1 ? `<p>Ook gemeten: ${T.ijkpunten.slice(0, -1).map((x) => `${esc(x.naam)}: ${x.gelijk} gelijk, ${x.verschillend} verschillend`).join("; ")}.</p>` : ""}</div>`;
  for (const [g, uitleg] of Object.entries(T.groepen)) {
    const rij = per[g] || [];
    if (!rij.length) continue;
    h += `<div class="groepkop"><h2>${esc(g[0].toUpperCase() + g.slice(1))} (${rij.length})</h2><p>${esc(uitleg)}</p></div>`;
    rij.sort((p, q) => p[0].localeCompare(q[0], "nl", { numeric: true }));
    for (const [nr, x] of rij) {
      const id = idVanNr(stand(N), nr) || idVanNr(stand(0), nr);
      const stappen = id ? RAAK.get(id) || [] : [];
      const raak = stappen.map((i) => STAPPEN[i - 1]).filter((s) => s.status !== "ok");
      h += `<section class="artikel"><div class="meta">Artikel ${esc(nr)}${stappen.length ? ` · ${stappen.length} handelingen` : " · door geen stuk geraakt"}${raak.map((s) => ` · <button class="naarstap" data-stap="${s.i}">nr. ${esc(nrVan(s.stuk))} ${esc(s.onderdeel)} ${STATUS[s.status][0]}</button>`).join("")}${id ? ` · <button class="naarart" data-art="${esc(id)}">toelichting</button>` : ""}</div>`;
      for (const d of regeldiff(alsRegels(x.eigen), alsRegels(x.bron))) {
        const cls = d.soort === "nieuw" ? " nieuw" : d.soort === "vervallen" ? " vervallen" : "";
        h += `<div class="diffregel${cls}">${d.html ?? esc(d.regel[0])}</div>`;
      }
      h += `</section>`;
    }
  }
  $("document").innerHTML = h;
  bouwArtikellijst(stand(Z.stap));
}

function tekenUitlegKeten() {
  const sluit = T.doorwerking.filter((d) => !d.versies[0].ijk.verschillend.length).length;
  $("document").innerHTML = `<section class="artikel uitleg" style="font:var(--primitives-font-body-md-regular-loose);padding:18px 22px">
  <h2 style="margin-top:0">${esc(T.naam)}</h2>
  <p>Elke versie van de wet die wetten.overheid.nl kent, van ${datumTekst(T.momenten.find((m) => m.stappen || m.stuk === VOORSTEL).datum)} tot nu, en daartussen de wetten die haar veranderden. De speler loopt langs die overgangen; op elk moment staat de wet zoals zij toen gold, met gemarkeerd wat de laatste overgang veranderde. De streep bij een regel en de herkomst rechts zeggen sinds welke overgang de regel zo luidt.</p>
  <p>Elke overgang is getoetst: de wijzigingsartikelen uit het Staatsblad van die wet zijn afgespeeld op de versie ervoor en naast de versie erna gelegd. Bij ${sluit} van de ${T.stappen.length} overgangen komt dat woord voor woord uit (groen); bij de andere zegt <b>Doorwerking</b> welke artikelen afwijken en waarom: een opdracht die deze grammatica (nog) niet leest, een wijziging die niet uit een wet komt (een tabel in een aanpassingswet, een besluit), een gefaseerde inwerkingtreding, of een echt verschil tussen Staatsblad en wetten.overheid.nl.</p>
  <p>Wat vóór de oudste versie op wetten.overheid.nl gebeurde, staat als moment in de tijd maar speelt niet af: de site toont geen oudere tekst. Wat nog aanhangig is, staat in het dossier zelf; kies het bovenin als het daar is.</p>
  <p>Gebouwd door <i>keten.py</i> uit de versies op wetten.overheid.nl en de Staatsbladen, opgehaald met <i>kamerdossier.py ${esc(T.dossier)} --keten</i>. Werk in uitvoering.</p></section>`;
  bouwArtikellijst(stand(Z.stap));
}

function tekenUitleg() {
  if (T.soort === "keten") return tekenUitlegKeten();
  const c = T.commentaar.tel;
  const stemmingen = T.momenten.filter((m) => m.soort === "stemming");
  const amendMee = T.stukken.filter((s) => s.soort.startsWith("Amendement") && s.speelt_mee).length;
  $("document").innerHTML = `<section class="artikel uitleg" style="font:var(--primitives-font-body-md-regular-loose);padding:18px 22px">
  <h2 style="margin-top:0">Wat deze pagina laat zien</h2>
  <p>${esc(T.naam)}, Kamerstuk ${esc(T.dossier)}: de tekst zoals de Kamerstukken haar maken, en wat er in die tijd over elk artikel werd gezegd. De speler loopt door de tijd: elk stuk is een moment op zijn datum (de bolletjes in de kalender; vierkant voor de Eerste Kamer, een ruit voor een stemming of een Staatsblad), en een nota van wijziging of een stemming speelt daarna haar handelingen af: ${N} in totaal, ${T.tel.afgespeeld} afgespeeld, ${T.tel.aanname || 0} met een aanname en ${T.tel.niet} niet.${stemmingen.length ? ` Bij een stemming spelen de aangenomen amendementen af (${amendMee}); een verworpen, ingetrokken of vervangen amendement staat bij zijn artikel, maar verandert de tekst niet.` : " Er is (nog) niet gestemd: amendementen staan bij hun artikel, maar veranderen de tekst niet."}</p>
  <p><b>Tekst</b> toont de wet op dat moment met gemarkeerd wat net veranderde; de streep links van een regel zegt uit welk stuk zij komt. Rechts staat het gekozen artikel door de tijd: het voorstel, de toelichting erbij, vragen en antwoorden, amendementen en elke wijziging, in de volgorde waarin ze verschenen, en onderaan het verschil met het eindpunt. Een artikel blijft hetzelfde artikel als het een ander nummer krijgt.</p>
  <p><b>Vergelijk</b> legt twee momenten naast elkaar, of een moment naast een ijkpunt: ${T.ijkpunten.length ? T.ijkpunten.map((x) => esc(x.naam)).join(", ") : "er is er geen"}. ${EIND ? `Het eindpunt is ${esc(EIND.naam)}.` : ""}</p>
  ${T.doorwerking.length ? `<p><b>Doorwerking</b>: dit voorstel wijzigt ${T.doorwerking.map((d) => esc(d.titel)).join(" en ")}. Elke officiële versie van het voorstel (het voorstel zelf, en het gewijzigd voorstel zoals de Tweede Kamer het aannam) is als opdracht op die wet afgespeeld: wat de wet wordt als het voorstel zo in werking treedt.</p>` : ""}
  <p><b>Controle</b> legt de laatste stand naast het eindpunt, ingedeeld naar wat een verschil waarschijnlijk is.</p>
  <h2>Hoe de toelichting aan een artikel hangt</h2>
  <p>${c.brokken} brokken uit ${c.gelezen.length} stukken. De sterkste haak is een kop: de memorie van toelichting heeft per artikel een eigen paragraaf (${c.artikelen_met_toelichting} artikelen), en ook verslagen en nota's naar aanleiding daarvan kunnen een artikelsgewijs deel hebben; samen ${c.artikelsgewijs} brokken. Een amendement hangt aan het artikel dat zijn opdracht noemt (${c.amendement}). Daarnaast ${c.vermelding} brokken waarin een alinea elders het artikel noemt; een vermelding van een andere regeling telt niet mee.${c.aan_de_doelwet ? ` ${c.aan_de_doelwet} brokken hangen (ook) aan een artikel van de wet die het voorstel wijzigt.` : ""} Een stuk telt in de nummering van zijn eigen datum.</p>
  <p>Het advies van de Raad van State hangt aan geen artikel: het noemt de nummering van de versie die de Afdeling zag. ${c.niet_gehaakt} alinea's zijn om die reden, of omdat het genoemde artikel in geen enkele stand bestaat, niet gehaakt.</p>
  <h2>Hoe een nota hier wordt gelezen</h2>
  <p>Een nota van wijziging leest als gelijktijdig: haar onderdelen citeren de tekst van vóór de nota. Gerichte opdrachten spelen daarom eerst, veegacties over veel artikelen daarna. Een opdracht die niet helemaal te lezen is, wordt niet half uitgevoerd maar gemeld (rood); wat met een aanname afspeelt is geel.</p>
  <h2>Herkomst van deze pagina</h2>
  <p>Gebouwd door <i>bouw-wetsgeschiedenis.py</i> uit een momentopname van de Kamerstukken van dossier ${esc(T.dossier)}, via <i>handelingen.py</i>, <i>afspelen.py</i> en <i>kamertoelichting.py</i>; een nieuw dossier haal je op met <i>kamerdossier.py</i>, ook vanaf een BWB-id. Een poort bewaakt dat de afspeler niet gaat gokken: wat hij niet kan lezen, speelt niet af. Werk in uitvoering.</p>
  </section>`;
  bouwArtikellijst(stand(Z.stap));
}

// ------------------------------------------------------------- de rail: het artikel door de tijd
function opdrachtHtml(t) { return esc(t).replace(/«([^«»]*)»/g, "<q>$1</q>"); }
function alineas(tekst, nr, toon) {
  const nrRx = nr ? nr.replace(/[.]/g, "\\.") : "";
  const merk = (t) => nr ? esc(t).replace(new RegExp(`\\b([Aa]rtikel(?:en)?[\\s\\u00a0]+(?:[0-9a-z.:]+(?:,[\\s\\u00a0]+|[\\s\\u00a0]+en[\\s\\u00a0]+))*${nrRx}(?![\\w.:]\\d)(?:,?[\\s\\u00a0]+[a-z]+[\\s\\u00a0]+lid)?)`, "g"), "<mark>$1</mark>") : esc(t);
  const p = (x) => `<p class="${x[1] === "kop" ? "tk" : x[1] === "li" ? "li" : ""}">${merk(x[0])}</p>`;
  if (tekst.length <= toon + 1) return tekst.map(p).join("");
  return tekst.slice(0, toon).map(p).join("") + `<details class="meer"><summary>Lees verder (${tekst.length - toon} alinea's)</summary>${tekst.slice(toon).map(p).join("")}</details>`;
}
function wanneer(stuk, extra) {
  const st = STUK[stuk] || {};
  return `<div class="wanneer"><b>${datumTekst(st.datum)}</b><span>nr. ${esc(nrVan(stuk))} · ${esc(st.soort || "")}${st.kamer === "Eerste Kamer" ? " (EK)" : ""}</span>${extra || ""}</div>`;
}
// Waar een brok vandaan komt, in een regel: bij een wet door de tijd is dat een
// nota van toelichting of een Kamerstuk van het dossier achter de overgang.
const bekendmaking = (x) => (x || "").replace(/^stb-(\d{4})-(\d+)$/, "Stb. $1, $2").replace(/^stcrt-(\d{4})-(\d+)$/, "Stcrt. $1, $2");
function bronTekst(b) {
  const x = b.bron;
  if (!x) return "";
  if (x.soort === "nota van toelichting") return `nota van toelichting, ${bekendmaking(x.publicatie)}`;
  return `Kamerstuk ${x.stuk.replace(/^kst-/, "")}, ${SOORTNAAM[b.soort] || b.soort}${x.datum ? ", " + datumTekst(x.datum) : ""}${x.oud ? `; in het voorstel artikel ${x.oud}` : ""}`;
}
// Een stuk van vóór 1995 is een scan met OCR: de link gaat naar het origineel.
const scanLink = (x) => x && /^https:\/\/zoek\.officielebekendmakingen\.nl\//.test(x.url || "") ? ` <a href="${esc(x.url)}" target="_blank" rel="noopener">scan</a>` : "";
const dossierKnop = (nr) => DOSSIERS.some((d) => d.dossier === nr) ? ` <button class="klein" data-dossier="${esc(nr)}">speel dossier ${esc(nr)} af</button>` : "";
function bronnenHtml(stuk, bronnen) {
  const alle = PERSTUK.get(stuk) || [];
  const algemeen = alle.filter((x) => x.rol === "overgang"), perArt = alle.filter((x) => x.rol !== "overgang");
  if (!bronnen && !alle.length) return "";
  const b = bronnen || {};
  const pub = (b.publicaties || []).map((p) => `<li>${esc(bekendmaking(p))}: ${b.nota && p in b.nota ? `nota van toelichting gelezen, ${b.nota[p]} ${b.nota[p] === 1 ? "brok" : "brokken"}` : "geen nota van toelichting gelezen"}${b.artikel && b.artikel[p] ? `; wijzigt deze regeling in artikel ${esc(b.artikel[p])}` : ""}</li>`);
  const dos = (b.dossiers || []).map((d) => `<li>Kamerdossier ${esc(d.nr)}: ${d.status === "opgehaald" ? `${d.brokken} ${d.brokken === 1 ? "brok" : "brokken"} over deze regeling${d.artikel ? ` (artikel ${esc(d.artikel)} van die wet)` : ""}${dossierKnop(d.nr)}`
    : d.status === "mislukt" ? `niet te lezen (${esc(d.reden || "onbekend")})` : d.status === "bezig" ? "wordt nu opgehaald" : d.status === "in de rij" ? "in de rij om op te halen" : "nog niet opgehaald"}</li>`);
  const ids = [...new Set(perArt.flatMap((x) => x.ids || []).filter(Boolean))];
  return `${pub.length || dos.length ? `<p class="klein">Waar de toelichting vandaan komt:</p><ul class="bronnen">${pub.join("")}${dos.join("")}</ul>` : ""}
    ${ids.length ? `<p class="klein">Toelichting bij ${ids.length} ${ids.length === 1 ? "artikel" : "artikelen"}:</p><div class="links">${ids.slice(0, 60).map((a) => `<button data-art="${esc(a)}">art. ${esc(nrVanId(a))}</button>`).join("")}</div>` : ""}
    ${algemeen.map((x) => `<details class="meer"><summary>${esc(x.titel || x.kop || "Toelichting")} <span class="zacht">· ${esc(bronTekst(x))}</span>${scanLink(x.bron)}</summary>${alineas(x.tekst, null, 99)}</details>`).join("")}`;
}
function brokKaart(b, nr) {
  const leden = (b.leden[nr] || []).map((l) => `<nldd-tag size="sm" color="neutral" text="lid ${l}"></nldd-tag>`).join("");
  if (b.rol === "amendement") {
    const st = STUK[b.stuk] || {};
    const u = st.uitslag ? st.uitslag.uitslag : null;
    const kleurU = u === "aangenomen" ? "success" : u ? "critical" : "neutral";
    return `<div class="kaart"><h4>${esc(b.kop)}</h4><div class="tags"><nldd-tag size="sm" color="neutral" text="amendement"></nldd-tag><nldd-tag size="sm" color="${kleurU}" text="${esc(uitslagTekst(st) || "niet gestemd")}"></nldd-tag>${st.speelt_mee ? `<nldd-tag size="sm" color="neutral" text="speelt af bij de stemming"></nldd-tag>` : ""}${b.nieuw.length ? `<nldd-tag size="sm" color="neutral" text="voegt in: ${esc(b.nieuw.join(", "))}"></nldd-tag>` : ""}</div>
      <details class="meer"><summary>De opdracht (${b.opdracht.length})</summary>${b.opdracht.map((o) => `<div class="opdracht">${opdrachtHtml(o)}</div>`).join("")}</details>
      ${alineas(b.tekst, nr, 2)}</div>`;
  }
  const titel = b.rol === "artikelsgewijs" ? (b.titel || `Artikel ${nr}`) : b.kop || "Elders in het stuk";
  const label = b.rol === "artikelsgewijs" ? "artikelsgewijs" : "noemt dit artikel";
  const doel = (b.doelartikelen || []).length ? `<nldd-tag size="sm" color="neutral" text="wijzigt art. ${esc(b.doelartikelen.join(", "))}"></nldd-tag>` : "";
  const bron = b.bron ? `<p class="klein zacht">${esc(bronTekst(b))}${scanLink(b.bron)}${b.bron.dossier ? dossierKnop(b.bron.dossier) : ""}</p>` : "";
  return `<div class="kaart"><h4>${esc(titel)}</h4><div class="tags"><nldd-tag size="sm" color="neutral" text="${label}"></nldd-tag>${leden}${doel}</div>${bron}${alineas(b.tekst, b.rol === "vermelding" ? nr : null, b.rol === "artikelsgewijs" ? 2 : 1)}</div>`;
}
function wijzigingKaart(id, stappen) {
  // De handelingen van één stuk op dit artikel, tot het moment in de speler.
  const getoond = stappen.filter((i) => i <= Z.stap);
  const reeks = getoond.length ? getoond : stappen;
  const [, voor] = artikelIn(stand(stappen[0] - 1), id);
  const [, na] = artikelIn(stand(reeks[reeks.length - 1]), id);
  let diff;
  if (voor && na) diff = (voor.nr !== na.nr ? `<div class="klein">Vernummerd: artikel ${esc(voor.nr)} wordt artikel ${esc(na.nr)}.</div>` : "") + miniDiff(voor.regels, na.regels);
  else if (na) diff = `<div class="klein">Het artikel komt erbij:</div>` + na.regels.map((r) => `<div class="diffregel nieuw">${esc(r[0])}</div>`).join("");
  else if (voor) diff = `<div class="klein">Het artikel vervalt.</div>`;
  else diff = `<div class="klein">Geen tekst om te tonen.</div>`;
  const ss = stappen.map((i) => STAPPEN[i - 1]);
  const raak = ss.filter((s) => s.status !== "ok");
  const koppen = [...new Set(ss.flatMap((s) => s.toelichting || []))];
  const toel = koppen.map((k) => { const p = T.toelichtingen[k] || []; return `<details class="meer"><summary>Toelichting bij ${esc(k)}</summary>${p.map((x) => `<p>${esc(x)}</p>`).join("") || `<p class="klein">Geen tekst gevonden bij deze kop.</p>`}</details>`; }).join("");
  return `<div class="kaart"><h4>${ss.length === 1 ? "Onderdeel" : "Onderdelen"} ${esc([...new Set(ss.map((s) => s.onderdeel))].join(", "))}</h4>
    <div class="tags">${raak.map((s) => `<nldd-tag size="sm" color="${STATUS[s.status][1]}" text="${esc(s.onderdeel)} ${STATUS[s.status][0]}"></nldd-tag>`).join("")}${getoond.length && getoond.length < stappen.length ? `<nldd-tag size="sm" color="neutral" text="${getoond.length} van ${stappen.length} handelingen tot hier"></nldd-tag>` : ""}</div>
    <div class="mini">${diff}</div>
    <div class="links">${stappen.map((i) => `<button data-stap="${i}" title="${esc(STAPPEN[i - 1].opdracht.slice(0, 160))}">${esc(STAPPEN[i - 1].onderdeel)}${STAPPEN[i - 1].sub ? " " + esc(STAPPEN[i - 1].sub) : ""}</button>`).join("")}</div>
    ${toel || `<p class="klein" style="margin-top:6px">Bij ${ss.length === 1 ? "dit onderdeel" : "deze onderdelen"} staat geen toelichting per onderdeel.</p>`}</div>`;
}
function draad(id) {
  const ev = [];
  if (artikelIn(stand(0), id)[1]) ev.push({ pos: POSSTUK[VOORSTEL] ?? 0, orde: 0, stuk: VOORSTEL, soort: "voorstel" });
  for (const b of PERART.get(id) || []) ev.push({ pos: POSSTUK[b.stuk] ?? 0, orde: b.rol === "artikelsgewijs" ? 1 : 3, stuk: b.stuk, soort: "brok", b });
  const perStuk = new Map();
  for (const i of RAAK.get(id) || []) { const st = STAPPEN[i - 1].stuk; if (!perStuk.has(st)) perStuk.set(st, []); perStuk.get(st).push(i); }
  for (const [st, stappen] of perStuk) ev.push({ pos: POSSTAP[stappen[0]], tot: POSSTAP[stappen[stappen.length - 1]], orde: 2, stuk: st, soort: "wijziging", stappen });
  ev.sort((a, b) => a.pos - b.pos || a.orde - b.orde || (a.b && b.b ? a.b.id - b.b.id : 0));
  return ev;
}
function gebeurtenisHtml(e, id) {
  const later = e.pos > Z.pos;
  const nu = !later && (e.soort === "wijziging" ? Z.pos >= e.pos && Z.pos <= e.tot : e.pos === Z.pos);
  const nr = nrVanId(id);
  let inhoud;
  if (e.soort === "voorstel") {
    const [, b] = artikelIn(stand(0), id);
    inhoud = `<div class="kaart"><h4>Ingediend</h4><details class="meer"><summary>Tekst van het voorstel</summary>${b.regels.map((r) => `<div class="diffregel${r[3] === "kop" ? " kop" : ""}">${esc(r[0])}</div>`).join("")}</details></div>`;
  } else if (e.soort === "brok") inhoud = brokKaart(e.b, e.b.artikelen[(e.b.ids || []).indexOf(id)] || nr);
  else inhoud = wijzigingKaart(id, e.stappen);
  const stem = e.soort === "wijziging" && TIJD[e.pos].moment.soort === "stemming" ? `<span class="uitslag">bij de stemming van ${datumTekst(TIJD[e.pos].moment.datum)}</span>` : "";
  return `<li class="gebeurtenis${nu ? " nu" : ""}${later ? " later" : ""}"><span class="punt" style="background:${kleur(e.stuk)}"></span>${wanneer(e.stuk, `${stem}<button class="klein" data-pos="${e.pos}">hierheen</button>`)}${inhoud}</li>`;
}
function eindpuntHtml(id) {
  if (!EIND) return "";
  const [, b] = artikelIn(stand(Z.stap), id);
  const nr = b ? b.nr : nrVanId(id);
  const e = EIND.artikelen[nr];
  let h = `<li class="gebeurtenis"><span class="punt" style="background:var(--k-ijkpunt)"></span><div class="wanneer"><b>Eindpunt</b><span>${esc(EIND.naam)}</span></div><div class="kaart">`;
  if (!e && !b) h += `<p class="klein">Staat op geen van beide plekken.</p>`;
  else if (!e) h += `<p>Dit artikel staat niet in het eindpunt.</p>`;
  else if (!b) h += `<p>Dit artikel bestaat op dit moment niet, maar wel in het eindpunt.</p>`;
  else {
    const x = genormd(b.regels), y = alsRegels(e);
    const gelijk = JSON.stringify(x.map((r) => r[0])) === JSON.stringify(y.map((r) => r[0]));
    const komt = (RAAK.get(id) || []).some((i) => i > Z.stap);
    h += gelijk ? `<div class="tags"><nldd-tag size="sm" color="success" text="gelijk aan het eindpunt"></nldd-tag></div><p class="klein">Op dit moment luidt het artikel woord voor woord als in het eindpunt.</p>`
      : `<div class="tags"><nldd-tag size="sm" color="${komt ? "neutral" : "warning"}" text="${komt ? "nog niet bij het eindpunt" : "wijkt af van het eindpunt"}"></nldd-tag>${!komt && T.bron[nr] ? `<nldd-tag size="sm" color="neutral" text="groep: ${esc(T.bron[nr].groep)}"></nldd-tag>` : ""}</div><div class="mini">${miniDiff(x, y)}</div><p class="klein">Rood is de tekst op dit moment, groen die van het eindpunt.${komt ? " Er komt later in de tijd nog een wijziging van dit artikel." : ""}</p>`;
  }
  return h + `<p class="klein">${esc(EIND.uitleg)}</p></div></li>`;
}
function herkomstHtml() {
  if (!Z.regel) return "";
  const S = stand(Z.stap);
  const b = S.b[Z.regel.sleutel];
  const r = b && b.regels[Z.regel.n];
  if (!r) return "";
  let h = `<h3>Herkomst van de gekozen regel <button class="klein" id="regel-weg">sluiten</button></h3><div class="kaart"><div class="opdracht" style="margin-bottom:8px">${esc(r[0].length > 220 ? r[0].slice(0, 220) + "…" : r[0])}</div><ul class="keten">`;
  for (const ref of r[1]) {
    const x = T.refs[ref], stap = REFSTAP.get(ref);
    h += `<li><span class="punt" style="background:${kleur(x.stuk)}"></span><span>${esc(refTekst(ref))} <span class="zacht">${datumTekst((STUK[x.stuk] || {}).datum)}</span>${stap ? ` <button class="naarstap" data-stap="${stap}">naar handeling ${stap}</button>` : ""}</span></li>`;
  }
  return h + `</ul></div>`;
}
function stapHtml() {
  const s = huidigeStap(), t = huidig();
  if (!s) {
    if (t.soort === "stemming") {
      const am = t.moment.stukken.map((x) => STUK[x]).filter(Boolean);
      return `<div class="kaart"><h4>Stemming in de Tweede Kamer, ${datumTekst(t.moment.datum)}</h4>${t.moment.voorstel ? `<div class="tags"><nldd-tag size="sm" color="${t.moment.voorstel === "aangenomen" ? "success" : "critical"}" text="het voorstel ${esc(t.moment.voorstel)}"></nldd-tag></div>` : ""}
        ${am.length ? `<p class="klein">Aangenomen amendementen, die hierna afspelen:</p><div class="links">${am.map((x) => `<button data-pos="${POSSTUK[x.stuk]}">nr. ${esc(x.ondernummer)}</button>`).join("")}</div>` : ""}</div>`;
    }
    if (t.soort === "ijkpunt") {
      const x = T.ijkpunten.find((y) => y.naam === t.moment.naam);
      return `<div class="kaart"><h4>${esc(t.moment.naam)}</h4><p class="klein">${esc(x ? x.uitleg : "")}</p>${x ? `<p class="klein">${x.gelijk} artikelen gelijk aan de afgespeelde tekst, ${x.verschillend} verschillend.</p>` : ""}</div>`;
    }
    const st = STUK[t.stuk] || {};
    const brokken = PERSTUK.get(t.stuk) || [];
    const ids = [...new Set(brokken.flatMap((b) => b.ids || []).filter(Boolean))];
    return `<div class="kaart"><h4>${esc(st.titel || st.soort || "")}</h4><div class="tags"><nldd-tag size="sm" color="neutral" text="${esc(SOORTNAAM[stukSoort(t.stuk)])}"></nldd-tag>${st.kamer === "Eerste Kamer" ? `<nldd-tag size="sm" color="neutral" text="Eerste Kamer"></nldd-tag>` : ""}${st.soort && st.soort.startsWith("Amendement") ? `<nldd-tag size="sm" color="neutral" text="${esc(uitslagTekst(st) || "niet gestemd")}"></nldd-tag>` : ""}</div>
      ${st.bronnen ? bronnenHtml(t.stuk, st.bronnen) : t.moment.stappen ? `<p class="klein">Hierna volgen de handelingen van deze nota.</p>` : ids.length ? `<p class="klein">Zegt iets over ${ids.length} ${ids.length === 1 ? "artikel" : "artikelen"}:</p><div class="links">${ids.slice(0, 60).map((a) => `<button data-art="${esc(a)}">art. ${esc(nrVanId(a))}</button>`).join("")}${ids.length > 60 ? `<span class="klein">en ${ids.length - 60} meer</span>` : ""}</div>` : `<p class="klein">Dit stuk hangt aan geen artikel.</p>`}</div>`;
  }
  const geraakt = STAPART[s.i - 1];
  if (s.soort === "versie") {
    // Een overgang van een wet door de tijd: wat veranderde, en waar de
    // toelichting op die verandering vandaan komt.
    return `<div class="kaart"><h4>${esc((STUK[s.stuk] || {}).titel || s.opdracht)}</h4>
      <div class="tags"><nldd-tag size="sm" color="${STATUS[s.status][1]}" text="${STATUS[s.status][0]}"></nldd-tag></div>
      <div class="opdracht">${opdrachtHtml(s.opdracht)}</div>
      ${s.reden ? `<div class="reden ${s.status}">${s.status === "niet" ? "Niet afgespeeld: " : "Aanname: "}${esc(s.reden)}</div>` : ""}
      ${bronnenHtml(s.stuk, s.bronnen)}</div>`;
  }
  return `<div class="kaart"><h4>Nr. ${esc(nrVan(s.stuk))}, onderdeel ${esc(s.onderdeel)}${s.sub ? " onder " + s.sub : ""}</h4>
    <div class="tags"><nldd-tag size="sm" color="${STATUS[s.status][1]}" text="${STATUS[s.status][0]}"></nldd-tag>${s.veeg ? `<nldd-tag size="sm" color="neutral" text="veegactie"></nldd-tag>` : ""}${s.ronde === 2 ? `<nldd-tag size="sm" color="neutral" text="tweede ronde"></nldd-tag>` : ""}</div>
    <div class="opdracht">${opdrachtHtml(s.opdracht)}</div>
    ${s.tekst && s.tekst.length ? `<div class="nieuwetekst">${s.tekst.map((x) => `<p>${esc(x)}</p>`).join("")}</div>` : ""}
    ${s.reden ? `<div class="reden ${s.status}">${s.status === "niet" ? "Niet afgespeeld: " : "Aanname: "}${esc(s.reden)}</div>` : ""}
    ${geraakt.length > 1 ? `<div class="links">${geraakt.slice(0, 40).map((id) => `<button data-art="${esc(id)}">art. ${esc(nrVanId(id))}</button>`).join("")}${geraakt.length > 40 ? `<span class="klein">en ${geraakt.length - 40} meer</span>` : ""}</div>` : ""}</div>`;
}
function tekenRail() {
  const id = Z.art;
  let kop, body = herkomstHtml();
  if (!id) {
    kop = `<h2>Kies een artikel</h2><div class="ondertitel">Klik op een artikel om het door de tijd te volgen.</div>`;
  } else {
    const [, b] = artikelIn(stand(Z.stap), id);
    const ooit = b || artikelIn(stand(N), id)[1] || artikelIn(stand(0), id)[1];
    const titel = ooit ? ooit.regels[0][0].replace(/^Artikel \S+\.?\s*/, "") : "";
    const ev = draad(id);
    const geweest = ev.filter((e) => e.pos <= Z.pos), komend = ev.filter((e) => e.pos > Z.pos);
    kop = `<h2>Artikel ${esc(ooit ? ooit.nr : nrVanId(id))}${titel ? `. ${esc(titel)}` : ""}</h2>
      <div class="ondertitel">${b ? "" : Z.stap < ((RAAK.get(id) || [Infinity])[0]) ? "Bestaat op dit moment nog niet. " : "Vervallen op dit moment. "}${geweest.length} gebeurtenissen tot ${datumTekst(datumVan(Z.pos))}${komend.length ? `, ${komend.length} daarna` : ""}</div>
      <div class="regelknoppen"><label class="filter" style="margin:0"><input type="checkbox" id="volg"${Z.volg ? " checked" : ""}> volg het afspelen</label><label class="filter" style="margin:0"><input type="checkbox" id="toekomst"${Z.toekomst ? " checked" : ""}> toon ook wat later komt</label></div>`;
    body += `<h3>Door de tijd</h3><ul class="draad">${geweest.map((e) => gebeurtenisHtml(e, id)).join("")}`;
    if (komend.length && Z.toekomst) body += komend.map((e) => gebeurtenisHtml(e, id)).join("");
    body += `</ul>`;
    if (komend.length && !Z.toekomst) body += `<div class="toekomst klein zacht">Nog ${komend.length} ${komend.length === 1 ? "gebeurtenis" : "gebeurtenissen"} later in de tijd: ${[...new Set(komend.map((e) => "nr. " + nrVan(e.stuk)))].join(", ")}.</div>`;
    body += `<ul class="draad">${eindpuntHtml(id)}</ul>`;
  }
  body += `<h3>Dit moment</h3>${stapHtml()}`;
  $("rail").innerHTML = `<div class="railkop">${kop}</div><div class="railbody">${body}</div>`;
  const v = $("volg"); if (v) v.onchange = (e) => { Z.volg = e.target.checked; if (Z.volg) { volgArtikel(); teken(true); } };
  const t = $("toekomst"); if (t) t.onchange = (e) => { Z.toekomst = e.target.checked; tekenRail(); };
  const w = $("regel-weg"); if (w) w.onclick = () => { Z.regel = null; document.querySelectorAll(".regel.gekozen").forEach((x) => x.classList.remove("gekozen")); tekenRail(); };
  const nuEl = $("rail").querySelector(".gebeurtenis.nu");
  if (nuEl && Z.speelt) nuEl.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
}

// ------------------------------------------------------------- kiezen en volgen
function volgArtikel() {
  const s = huidigeStap();
  if (s) { const a = STAPART[s.i - 1]; if (a.length) Z.art = a[0]; return; }
  const b = (PERSTUK.get(huidig().stuk) || []).filter((x) => (x.ids || []).some(Boolean));
  const a = b.find((x) => x.rol === "artikelsgewijs") || b[0];
  if (a) Z.art = a.ids.find(Boolean);
}
function kiesArtikel(id, scroll) {
  Z.art = id; Z.volg = false;
  if (Z.modus !== "tekst") zetModus("tekst");
  else {
    document.querySelectorAll(".artikel.focus").forEach((x) => x.classList.remove("focus"));
    document.querySelectorAll(`.artikel[data-art="${id}"]`).forEach((x) => x.classList.add("focus"));
    document.querySelectorAll(".toelknop.aan").forEach((x) => x.classList.remove("aan"));
    document.querySelectorAll(`.toelknop[data-art="${id}"]`).forEach((x) => x.classList.add("aan"));
    bouwArtikellijst(stand(Z.stap));
  }
  tekenRail();
  bewaarHash();
  if (scroll) scrollNaar(id);
}

function naarDossier(id) {
  try { localStorage.setItem("wg-dossier", id); } catch (x) {}
  location.hash = "d=" + id;
  location.reload();
}
$("rail").addEventListener("click", (e) => {
  const d = e.target.closest("[data-dossier]"); if (d) return naarDossier(d.dataset.dossier);
  const p = e.target.closest("[data-pos]"); if (p) { stop(); Z.volg = false; return ga(+p.dataset.pos); }
  const st = e.target.closest("[data-stap]"); if (st) { stop(); Z.volg = false; return ga(POSSTAP[+st.dataset.stap]); }
  const a = e.target.closest("[data-art]"); if (a) kiesArtikel(a.dataset.art, true);
});
$("document").addEventListener("click", (e) => {
  const st = e.target.closest("[data-stap]"); if (st) { stop(); Z.regel = null; zetModus("tekst"); return ga(POSSTAP[+st.dataset.stap]); }
  const na = e.target.closest(".naarart, .toelknop"); if (na) { stop(); return kiesArtikel(na.dataset.art, Z.modus !== "tekst"); }
  const r = e.target.closest(".regel[data-n]");
  const art = e.target.closest(".artikel[data-art]");
  if (r && r.dataset.n !== "-1" && Z.modus === "tekst") {
    Z.regel = { sleutel: r.dataset.sleutel, n: +r.dataset.n };
    document.querySelectorAll(".regel.gekozen").forEach((x) => x.classList.remove("gekozen"));
    r.classList.add("gekozen");
  }
  if (art && Z.modus === "tekst") kiesArtikel(art.dataset.art, false);
});

// ------------------------------------------------------------- besturing
function bewaarHash() { try { history.replaceState(null, "", `#d=${KEUZE}&t=${Z.pos}${Z.art ? "&art=" + Z.art : ""}`); } catch (e) {} }
function teken(scroll) {
  tekenNaald();
  bouwStukken();
  if (Z.modus === "tekst") tekenTekst();
  else if (Z.modus === "verschil") tekenVerschil();
  else if (Z.modus === "doorwerking") tekenDoorwerking();
  else if (Z.modus === "controle") tekenControle();
  else tekenUitleg();
  tekenRail();
  if (scroll && Z.modus === "tekst") {
    const s = huidigeStap();
    const doel = s ? (Object.keys(s.wijzig).map((k) => document.querySelector(`#document [data-sleutel="${k}"]`)).find(Boolean)) : null;
    if (doel) doel.scrollIntoView({ block: "start", inline: "nearest", behavior: Z.speelt ? "smooth" : "auto" });
    else if (Z.art) scrollNaar(Z.art, !!Z.speelt);
  }
  bewaarHash();
}
function ga(p) {
  p = Math.max(0, Math.min(P, p));
  if (p === Z.pos) { tekenNaald(); return; }
  Z.regel = null;
  Z.pos = p;
  if (Z.volg) volgArtikel();
  teken(true);
}
function raakt(p) {
  const t = TIJD[p], w = $("sprong").value;
  if (w === "alles") return true;
  if (w === "stukken") return t.soort !== "stap";
  if (w === "raak") return t.soort === "stap" && STAPPEN[t.stap - 1].status !== "ok";
  if (!Z.art) return true;
  if (t.soort === "stap") return STAPART[t.stap - 1].includes(Z.art);
  return (PERSTUK.get(t.stuk) || []).some((b) => (b.ids || []).includes(Z.art));
}
function volgende(r) {
  let p = Z.pos + r;
  while (p > 0 && p < P && !raakt(p)) p += r;
  return p;
}
function stop() {
  if (Z.speelt) { clearTimeout(Z.speelt); Z.speelt = null; }
  $("k-speel").setAttribute("icon", "media-play"); $("k-speel").setAttribute("text", "Afspelen");
}
function speel() {
  if (Z.speelt) return stop();
  if (Z.modus !== "tekst" && Z.modus !== "verschil") zetModus("tekst");
  if (Z.pos >= P) ga(0);
  $("k-speel").setAttribute("icon", "media-pause"); $("k-speel").setAttribute("text", "Pauzeren");
  const tik = () => {
    if (Z.pos >= P) return stop();
    ga(volgende(1));
    // Op een stuk met toelichting, een stemming of een ijkpunt blijft de speler wat langer staan.
    const t = huidig();
    const lang = t.soort === "stemming" || t.soort === "ijkpunt" || (t.soort === "stuk" && (PERSTUK.get(t.stuk) || []).length);
    Z.speelt = setTimeout(tik, +$("tempo").value * (lang ? 2.5 : 1));
  };
  Z.speelt = setTimeout(tik, 50);
}
function zetModus(m) {
  if (m === "doorwerking" && !T.doorwerking.length) m = "tekst";
  Z.modus = m;
  const sc = $("modus"); if (sc.value !== m) { sc.value = m; sc.setAttribute("value", m); }
  try { localStorage.setItem("wg-modus", m); } catch (e) {}
  teken(false);
}
$("k-begin").addEventListener("click", () => { stop(); ga(0); });
$("k-eind").addEventListener("click", () => { stop(); ga(P); });
$("k-terug").addEventListener("click", () => { stop(); ga(volgende(-1)); });
$("k-vooruit").addEventListener("click", () => { stop(); ga(volgende(1)); });
$("k-speel").addEventListener("click", speel);
$("k-stuk-terug").addEventListener("click", () => { stop(); let p = Z.pos - 1; while (p > 0 && TIJD[p].soort === "stap") p--; ga(p); });
$("k-stuk-vooruit").addEventListener("click", () => { stop(); let p = Z.pos + 1; while (p < P && TIJD[p].soort === "stap") p++; ga(p); });
$("tempo").addEventListener("change", () => { if (Z.speelt) { stop(); speel(); } });
$("modus").addEventListener("change", (e) => zetModus((e.detail && e.detail.value) || e.target.value));
$("zoek").addEventListener("input", (e) => { Z.zoek = (e.target.value || "").trim(); bouwArtikellijst(stand(Z.stap)); });
$("alleen-gewijzigd").addEventListener("change", (e) => { Z.alleenGewijzigd = e.target.checked; bouwArtikellijst(stand(Z.stap)); });
$("strook").addEventListener("keydown", (e) => {
  if (e.key === "Home") { stop(); ga(0); e.preventDefault(); }
  if (e.key === "End") { stop(); ga(P); e.preventDefault(); }
});
document.addEventListener("keydown", (e) => {
  if (e.target.closest && e.target.closest("input, select, textarea, nldd-search-field")) return;
  if (e.key === "ArrowRight") { stop(); ga(volgende(1)); e.preventDefault(); }
  else if (e.key === "ArrowLeft") { stop(); ga(volgende(-1)); e.preventDefault(); }
  else if (e.key === " ") { speel(); e.preventDefault(); }
});
window.addEventListener("resize", () => tekenNaald());

if (!T.doorwerking.length) { const it = $("modus-doorwerking"); if (it) it.remove(); }
{
  const data = MOMENTEN.map(([t]) => t.moment.datum).filter(Boolean).sort();
  $("titel").setAttribute("supporting-text", `${T.soort === "keten" ? "BWB " + T.dossier : "Kamerstuk " + T.dossier.replace(/(\d{2})(\d{3})/, "$1 $2")} · ${datumTekst(data[0])} tot ${datumTekst(data[data.length - 1])}`);
}
bouwKalender();
bouwStrook();
if (!Z.art) Z.art = idVanNr(stand(0), (T.begin.blokken[T.begin.volgorde.find((k) => T.begin.blokken[k].soort === "artikel")] || {}).nr) || null;
zetModus(Z.modus);

// ------------------------------------------------------------- een wet ophalen
// Alleen als er een dienst achter de pagina staat (de poc, dienst.py); het
// losse bestand heeft er geen, en dan blijft de knop verborgen.
(async () => {
  // Het losse bestand draagt zijn gegevens zelf en vraagt niets van buiten.
  if (document.getElementById("dossiers")) return;
  // In de Vue-vorm is het venster een eigen component (OphaalVenster.vue).
  if (!document.getElementById("ophaal-uit")) return;
  let st = null;
  try { const r = await fetch(BASIS + "api/status"); if (r.ok) st = await r.json(); } catch (e) { return; }
  if (!st || !st.ophalen) return;
  $("ophalen-plek").classList.remove("verborgen");
  const dlg = $("ophaal"), uit = $("ophaal-uit");
  const melding = (t) => uit.insertAdjacentHTML("afterbegin", `<p class="ophaal-fout">${esc(t)}</p>`);
  const fout = (t) => { uit.innerHTML = ""; melding(t + " Diagnose laat zien wat de dienst wel en niet bereikt."); };
  // Elk antwoord als {ok, status, x, s}. Een fout van het portaal (platte tekst)
  // of van de ingress (HTML) is geen JSON; dan zegt de melding wie antwoordde.
  async function api(pad, opties) {
    const t0 = performance.now();
    let r;
    try { r = await fetch(BASIS + pad, opties); }
    catch (e) { return { ok: false, status: 0, x: { fout: `Geen verbinding met de dienst (${e.message}).` } }; }
    const tekst = await r.text(), s = Math.round((performance.now() - t0) / 100) / 10;
    try { return { ok: r.ok, status: r.status, x: JSON.parse(tekst), s }; } catch (e) { /* geen JSON */ }
    const wie = r.status === 502 ? "Het portaal kreeg geen antwoord van de dienst"
      : r.status === 504 ? "De ingress wachtte te lang op de dienst" : "Een tussenliggende server antwoordde";
    const kort = tekst.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim().slice(0, 160);
    return { ok: false, status: r.status, s, x: { fout: `${wie} (HTTP ${r.status}, na ${s} s)${kort ? ": " + kort : "."}` } };
  }
  // Een teller zolang er gewacht wordt; de opzoeking kan tientallen seconden duren.
  function wacht(tekst) {
    const t0 = performance.now();
    uit.innerHTML = `<p>${esc(tekst)} <span id="ophaal-tijd"></span></p>`;
    const klok = setInterval(() => {
      const el = $("ophaal-tijd");
      if (el) el.textContent = Math.round((performance.now() - t0) / 1000) + " s"; else clearInterval(klok);
    }, 1000);
    return () => clearInterval(klok);
  }
  const regel = (r) => [r.t.slice(11), r.soort, r.methode, r.wat && `${r.wat} ${r.bwb}${r.dossier ? " " + r.dossier : ""}`,
    r.host ? r.host + r.pad : r.pad || r.bij, r.status ?? r.http ?? (r.code ? "curl " + r.code : ""),
    r.uitleg || r.fout || r.oordeel, r.ms != null ? r.ms + " ms" : r.s != null ? r.s + " s" : ""].filter((v) => v != null && v !== "").join("  ");
  const open = (id) => { try { localStorage.setItem("wg-dossier", id); } catch (e) {} location.hash = "d=" + id; location.reload(); };
  $("ophalen").addEventListener("click", () => { dlg.showModal(); if (!gevolgd && st.taak && st.taak.status === "bezig") volg(st.taak.id); });
  $("ophaal-sluit").addEventListener("click", () => dlg.close());
  async function zoek() {
    const bwb = $("ophaal-bwb").value.trim().toUpperCase();
    if (!/^BWBR\d{7}$/.test(bwb)) { uit.innerHTML = ""; return melding("Een BWB-id ziet eruit als BWBR0007376."); }
    const klaar = wacht(`Op wetten.overheid.nl opzoeken wat ${bwb} maakte en wijzigde …`);
    const { ok, x: reg } = await api("api/regeling/" + bwb);
    klaar();
    if (!ok) return fout(reg.fout);
    const knop = (id, tekst, attr) => `<button type="button" class="ophaal-actie" data-id="${esc(id)}" ${attr}>${esc(tekst)}</button>`;
    const stand = (d) => d.beschikbaar ? "" : d.rij === "bezig" ? " (wordt nu opgehaald)" : d.rij ? " (in de rij)" : d.mislukt ? " (eerder mislukt)" : "";
    const nDossiers = new Set(reg.versies.flatMap((v) => v.dossiers.map((d) => d.nr))).size;
    uit.innerHTML = `<h3>${esc(reg.titel)}${reg.afkorting ? ` (${esc(reg.afkorting)})` : ""}</h3>
      <p>${knop(reg.bwb, reg.keten ? "Haal de wet opnieuw op" : "Haal de wet op", 'data-keten="1"')}${reg.keten ? " " + knop(reg.bwb, "Open", 'data-open="1"') : ""}</p>
      <p class="klein">Elke versie op wetten.overheid.nl, elke overgang afgespeeld uit haar bekendmaking, met de nota van toelichting die daarin staat.
        ${nDossiers ? `Daarna haalt de dienst op de achtergrond de ${nDossiers} Kamerdossiers hieronder op, de jongste eerst; elk dossier dat klaar is, hangt zijn toelichting aan zijn overgang. Een dossier kun je ook los ophalen en afspelen.`
          : "Geen versie kwam uit een Kamerdossier: een algemene maatregel van bestuur of een ministeriële regeling gaat niet langs de Kamer, en haar toelichting staat in haar eigen bekendmaking."}</p>
      ${reg.versies.length ? `<table class="ophaal-lijst"><thead><tr><th>In werking</th><th>Wat</th><th>Bekendmaking</th><th>Kamerdossier</th></tr></thead><tbody>
      ${reg.versies.map((v) => `<tr><td>${v.inwerking ? datumTekst(v.inwerking) : "nog niet"}</td><td>${esc(v.betreft || "")}</td>
        <td>${esc(bekendmaking(v.staatsblad || ""))}</td><td>${v.dossiers.map((d) => knop(d.nr, (d.beschikbaar ? "Open " : "Haal nu op: ") + d.nr + stand(d), d.beschikbaar ? 'data-open="1"' : "")).join("")}</td></tr>`).join("")}
      </tbody></table>` : ""}`;
    uit.querySelectorAll(".ophaal-actie").forEach((b) => b.addEventListener("click", async () => {
      if (b.dataset.open) return open(b.dataset.id);
      const vraag = b.dataset.keten ? { bwb: reg.bwb, keten: true } : { bwb: reg.bwb, dossier: b.dataset.id };
      const { ok, x: a } = await api("api/haal", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(vraag) });
      if (!ok) return melding(a.fout);
      volg(a.id);
    }));
  }
  $("ophaal-zoek").addEventListener("click", zoek);
  $("ophaal-bwb").addEventListener("keydown", (e) => { if (e.key === "Enter") zoek(); });
  // Een opdracht volgen op haar id: ook als zij nog in de rij staat, en met wat
  // de rij daarna nog voor dezelfde wet ophaalt.
  let gevolgd = null;
  const korteNaam = (x) => x.wat === "keten" ? `${x.bwb} door de tijd` : `dossier ${x.dossier}`;
  async function volg(id) {
    if (id) gevolgd = id;
    const [{ ok, x: t }, { x: stand }] = await Promise.all([api("api/taak" + (gevolgd ? "?id=" + encodeURIComponent(gevolgd) : "")), api("api/status")]);
    if (!ok) return fout(t.fout);
    if (!t) return;
    const rij = (stand && stand.rij) || { huidig: null, wachtend: [] };
    const wet = t.wat === "keten" ? t.bwb : t.voor;
    const achter = [rij.huidig, ...rij.wachtend].filter((x) => x && x.voor && x.voor === wet && x.id !== t.id);
    uit.innerHTML = `<h3>${esc({ wacht: "In de rij: ", bezig: "Bezig: ", klaar: "Klaar: ", fout: "Mislukt: " }[t.status] + korteNaam(t))}</h3>`
      + (t.status === "wacht" ? `<p>Plaats ${t.plaats} in de rij${rij.huidig ? `; nu bezig: ${esc(korteNaam(rij.huidig))}` : ""}.</p>` : "")
      + (t.fout ? `<p class="ophaal-fout">${esc(t.fout)}</p>` : "")
      + (t.status === "klaar" ? `<p><button type="button" class="ophaal-actie" id="ophaal-open">Open ${esc(t.uitkomst)}</button></p>` : "")
      + (t.status === "bezig" ? "<p>De stukken ophalen en afspelen duurt een paar minuten.</p>" : "")
      + (achter.length ? `<p class="klein">Op de achtergrond nog ${achter.length} Kamerdossier${achter.length === 1 ? "" : "s"} voor ${esc(wet)}: ${achter.map((x) => esc(x.dossier) + (x.status === "bezig" ? " (nu)" : "")).join(", ")}. Je kunt de wet al openen; wat later klaar is, staat erin als je de pagina opnieuw laadt.</p>` : "")
      + (t.log.length ? `<pre class="ophaal-log">${esc(t.log.join("\n"))}</pre>` : "");
    if (t.status === "klaar") $("ophaal-open").addEventListener("click", () => open(t.uitkomst));
    if (t.status === "bezig" || t.status === "wacht" || achter.length) setTimeout(volg, 1500);
  }
  // Wat de dienst bereikt en wat hij de laatste tijd deed. Met een BWB-id in het
  // veld toetst hij ook de opzoeking zelf.
  async function diagnose() {
    const bwb = $("ophaal-bwb").value.trim().toUpperCase(), met = /^BWBR\d{7}$/.test(bwb);
    const klaar = wacht(`De dienst toetst wat hij bereikt${met ? " en zoekt " + bwb + " op" : ""} …`);
    const [{ ok, x: d }, { x: log }] = await Promise.all([api("api/diagnose" + (met ? "?bwb=" + bwb : "")), api("api/log")]);
    klaar();
    if (!ok) return fout(d.fout);
    const proxy = Object.entries(d.proxy).map(([k, v]) => `${k}=${v}`).join(", ") || "geen";
    uit.innerHTML = `<h3 class="${d.oordeel === "ophalen kan" ? "" : "ophaal-fout"}">${esc(d.oordeel)}</h3>
      <table class="ophaal-lijst"><thead><tr><th>Host</th><th>Bereikbaar</th><th>Waarvoor</th></tr></thead><tbody>
      ${Object.entries(d.hosts).map(([h, v]) => `<tr><td>${esc(h)}</td><td>${v.ok ? `ja, HTTP ${esc(v.http)} in ${esc(v.totaal_s)} s`
        : `<span class="ophaal-fout">nee: ${esc(v.uitleg || v.fout || "onbekend")}</span>`}</td><td>${esc(v.waarvoor)}</td></tr>`).join("")}
      </tbody></table>
      <p>Schrijven in ${esc(d.data.pad)}: ${d.data.schrijfbaar ? `ja, ${esc(d.data.vrij_mb)} MB vrij` : `<span class="ophaal-fout">nee: ${esc(d.data.fout)}</span>`}.
        ${esc(d.curl)}. Proxy: ${esc(proxy)}. Dossiers: ${d.dossiers.in_het_image} in het image, ${d.dossiers.opgehaald} opgehaald (ten hoogste ${d.dossiers.max}).</p>
      ${d.opzoeking ? `<p>Opzoeking van ${esc(d.opzoeking.bwb)}: ${d.opzoeking.ok ? `${esc(d.opzoeking.titel)}, ${d.opzoeking.versies_met_dossier} versies met een Kamerdossier`
        : `<span class="ophaal-fout">${esc(d.opzoeking.fout)}</span>`} (${d.opzoeking.s} s).</p>` : ""}
      <h3>Logboek</h3>
      <pre class="ophaal-log">${esc((Array.isArray(log) ? log : []).slice(-40).map(regel).join("\n") || "leeg")}</pre>
      <p>Als JSON, om door te sturen: <a href="${BASIS}api/diagnose${met ? "?bwb=" + bwb : ""}" target="_blank" rel="noopener">api/diagnose</a>,
        <a href="${BASIS}api/log" target="_blank" rel="noopener">api/log</a>.</p>`;
  }
  $("ophaal-diagnose").addEventListener("click", diagnose);
})();
