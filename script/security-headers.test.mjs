// Pint vast dat elk RegelRecht-domein dezelfde security-headers stuurt, en dat
// er geen plek is die ze stilletjes overslaat.
//
// De aanleiding: er was een CSP, hij stond netjes in de diff, en de browser
// kreeg hem nooit — één keer omdat het bestand door geen enkele Dockerfile
// gelezen werd, één keer omdat de middleware vóór de `fallback_service` hing
// die de SPA serveert. Zulke fouten maken niets rood. Ze moeten hier rood
// worden.
//
// De drie servertypes lezen elk hun eigen bron: de Axum-diensten
// `packages/auth/src/security_headers.rs`, de nginx-images
// `deploy/nginx/security-headers.conf`, Grafana zijn `GF_SECURITY_*`. Deze
// test is de enige plek waar die drie naast elkaar liggen.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const root = fileURLToPath(new URL('../', import.meta.url));
const read = (path) => readFileSync(join(root, path), 'utf8');

const SHARED_PATH = 'deploy/nginx/security-headers.conf';
const SHARED_TARGET = '/etc/nginx/security-headers.conf';

const RUST = read('packages/auth/src/security_headers.rs');
const SHARED = read(SHARED_PATH);

// Zoeken in plaats van opsommen: een derde site die zijn eigen nginx.conf
// meebrengt hoort onder dezelfde eisen te vallen zonder dat iemand hem hier
// toevoegt. Een `nginx.conf.template` telt mee: het image van nginx vult er bij
// het starten omgevingsvariabelen in, verder is het dezelfde configuratie.
function findNginxConfigs(dir = '', found = []) {
  const skip = new Set(['node_modules', '.git', 'target', 'dist', '.worktrees']);
  for (const entry of readdirSync(join(root, dir))) {
    if (skip.has(entry) || entry.startsWith('.')) continue;
    const rel = dir ? `${dir}/${entry}` : entry;
    if (statSync(join(root, rel)).isDirectory()) findNginxConfigs(rel, found);
    else if (entry === 'nginx.conf' || entry === 'nginx.conf.template') found.push(rel);
  }
  return found;
}

const NGINX_SITES = Object.fromEntries(findNginxConfigs().map((p) => [p, read(p)]));

/// De header-naam zoals nginx hem schrijft, gekoppeld aan de Rust-constante.
/// De CSP staat er niet bij: die beschrijft één document en verschilt per site.
const SHARED_HEADERS = {
  'X-Content-Type-Options': 'X_CONTENT_TYPE_OPTIONS',
  'X-Frame-Options': 'X_FRAME_OPTIONS',
  'Referrer-Policy': 'REFERRER_POLICY',
  'Permissions-Policy': 'PERMISSIONS_POLICY',
  'Strict-Transport-Security': 'STRICT_TRANSPORT_SECURITY',
};

function rustConstant(name) {
  const match = RUST.match(new RegExp(`pub const ${name}: &str = "([^"]*)"`));
  assert.ok(match, `constante ${name} niet gevonden in security_headers.rs`);
  return match[1];
}

/// De `\` aan het regeleinde plakt de string in Rust aan elkaar zonder de
/// witruimte ervoor; hier doen we hetzelfde.
function rustCsp(name) {
  const match = RUST.match(new RegExp(`pub const ${name}: &str = "([\\s\\S]*?)";`));
  assert.ok(match, `constante ${name} niet gevonden in security_headers.rs`);
  return match[1].replace(/\\\s+/g, '');
}

/// Splits een config in server-blokken, en elk server-blok in zijn
/// location-blokken. Beide niveaus zijn nodig: `set $csp` hoort per server,
/// de include per location.
function serverBlocks(conf) {
  const servers = [];
  const re = /server\s*\{/g;
  let match;
  while ((match = re.exec(conf)) !== null) {
    let depth = 1;
    let i = re.lastIndex;
    while (i < conf.length && depth > 0) {
      if (conf[i] === '{') depth += 1;
      else if (conf[i] === '}') depth -= 1;
      i += 1;
    }
    servers.push(conf.slice(re.lastIndex, i - 1));
  }
  return servers;
}

function locationBlocks(server) {
  const blocks = [];
  const re = /location\s+([^{]+)\{/g;
  let match;
  while ((match = re.exec(server)) !== null) {
    let depth = 1;
    let i = re.lastIndex;
    while (i < server.length && depth > 0) {
      if (server[i] === '{') depth += 1;
      else if (server[i] === '}') depth -= 1;
      i += 1;
    }
    blocks.push({ selector: match[1].trim(), body: server.slice(re.lastIndex, i - 1) });
  }
  return blocks;
}

function directive(csp, name) {
  return csp
    .split(';')
    .map((d) => d.trim())
    .find((d) => d.split(/\s+/)[0] === name);
}

test('er is iets om te vergelijken', () => {
  // Een lege verzameling zou elke test hieronder leeg laten slagen.
  assert.ok(Object.keys(NGINX_SITES).length >= 2, Object.keys(NGINX_SITES).join(', '));
});

test('nginx stuurt exact de waarden die de Rust-diensten sturen', () => {
  for (const [header, constant] of Object.entries(SHARED_HEADERS)) {
    const expected = rustConstant(constant);
    const match = SHARED.match(new RegExp(`add_header ${header} "([^"]*)" always;`));
    assert.ok(match, `${header} ontbreekt in ${SHARED_PATH}`);
    assert.equal(match[1], expected, `${header} loopt uiteen tussen nginx en security_headers.rs`);
  }
});

test('het gedeelde bestand stuurt ook een CSP', () => {
  assert.match(SHARED, /add_header Content-Security-Policy \$csp always;/);
});

test('elk server-blok zet zijn eigen $csp', () => {
  for (const [name, conf] of Object.entries(NGINX_SITES)) {
    for (const [index, server] of serverBlocks(conf).entries()) {
      const match = server.match(/set \$csp "([^"]*)";/);
      // Zonder eigen `set` erft een tweede server-blok de variabele niet: hij
      // is dan leeg, en nginx laat de header dan zonder klacht weg.
      assert.ok(match, `${name}: server-blok ${index} zet geen $csp`);
      const csp = match[1];
      for (const [directiveName, allowed] of [
        ['frame-ancestors', ["'none'"]],
        ['base-uri', ["'none'"]],
        ['form-action', ["'none'", "'self'"]],
      ]) {
        const found = directive(csp, directiveName);
        assert.ok(found, `${name}: ${directiveName} ontbreekt in de CSP`);
        assert.ok(
          allowed.includes(found.slice(directiveName.length).trim()),
          `${name}: ${found}`,
        );
      }
      assert.ok(!csp.includes("'unsafe-eval'"), `${name}: 'unsafe-eval' in de CSP`);
    }
  }
});

test('elk location-blok behalve de healthprobe draagt de headers', () => {
  for (const [name, conf] of Object.entries(NGINX_SITES)) {
    for (const server of serverBlocks(conf)) {
      for (const { selector, body } of locationBlocks(server)) {
        // De healthprobe is de enige uitzondering: geen document, geen
        // publiek pad. Redirects horen er wél bij — de security.txt-route is
        // juist het pad dat een scanner aanloopt.
        if (selector === '/health') continue;
        assert.ok(
          body.includes(`include ${SHARED_TARGET};`),
          `${name}: location ${selector} antwoordt zonder de headers`,
        );
      }
    }
  }
});

test('elk nginx-image kopieert het gedeelde bestand mee', () => {
  for (const site of Object.keys(NGINX_SITES)) {
    // Zonder de COPY faalt nginx pas bij containerstart op de ontbrekende
    // include, dus in deploy-preview in plaats van hier.
    const dockerfile = read(`${site.replace(/\/nginx\.conf(\.template)?$/, '')}/Dockerfile`);
    assert.ok(
      dockerfile.includes(`COPY ${SHARED_PATH} ${SHARED_TARGET}`),
      `${site}: de Dockerfile kopieert ${SHARED_PATH} niet`,
    );
  }
});

test('geen enkele nginx-CSP staat inline script toe, op docs na', () => {
  // `docs` houdt bewust `script-src 'unsafe-inline'`: Astro emitteert een
  // pre-paint theme-script en een Pagefind-script waarvan de inhoud per
  // pagina verschilt, dus één nginx-brede hash volstaat niet.
  const withInlineScript = ['docs/nginx.conf'];
  for (const [name, conf] of Object.entries(NGINX_SITES)) {
    if (withInlineScript.includes(name)) continue;
    for (const server of serverBlocks(conf)) {
      const csp = server.match(/set \$csp "([^"]*)";/)[1];
      const governing = directive(csp, 'script-src') ?? directive(csp, 'default-src');
      assert.ok(governing, `${name}: script valt onder geen enkele directive`);
      assert.ok(!governing.includes("'unsafe-inline'"), `${name}: ${governing}`);
    }
  }
});

test('de editor-CSP laat de wasm-engine daadwerkelijk starten', () => {
  // `useEngine.js` haalt de wasm-bindgen-glue op, stopt hem in een Blob en
  // `import()`t die URL; de glue roept vervolgens WebAssembly.instantiate aan.
  // Zonder deze twee waarden start de engine niet, en dat merk je pas in de
  // browser — geen enkele Rust-test compileert wasm.
  const scriptSrc = directive(rustCsp('EDITOR_CSP'), 'script-src');
  assert.ok(scriptSrc, 'EDITOR_CSP heeft geen script-src');
  assert.ok(scriptSrc.includes("'wasm-unsafe-eval'"), scriptSrc);
  assert.ok(scriptSrc.includes('blob:'), scriptSrc);
});

test('grafana zet de headers aan die het kent', () => {
  const dockerfile = read('packages/grafana/Dockerfile');
  for (const setting of [
    'GF_SECURITY_STRICT_TRANSPORT_SECURITY=true',
    'GF_SECURITY_CONTENT_SECURITY_POLICY=true',
  ]) {
    assert.ok(dockerfile.includes(setting), `${setting} ontbreekt`);
  }
  // Grafana's eigen template kent geen frame-ancestors; zonder die toevoeging
  // leunt het embedden-verbod alleen op X-Frame-Options.
  assert.match(dockerfile, /GF_SECURITY_CONTENT_SECURITY_POLICY_TEMPLATE=.*frame-ancestors 'none'/);
  // De nonce moet de Dockerfile-parser overleven; ingevuld op buildtijd valt
  // 'strict-dynamic' terug op niets.
  assert.match(dockerfile, /\\\$NONCE/);
});
