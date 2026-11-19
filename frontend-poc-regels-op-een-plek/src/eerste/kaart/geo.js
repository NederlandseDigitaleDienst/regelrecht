// Meetkunde voor de kaart: punt-in-vlak op GeoJSON in WGS84, en RD-coördinaten.
// Dit is meetkunde, geen recht: welke regel geldt, bepaalt stapel.js met de gelaagdheidsregel
// uit de bron.
import proj4 from 'proj4';

proj4.defs('EPSG:28992',
  '+proj=sterea +lat_0=52.15616055555555 +lon_0=5.38763888888889 +k=0.9999079 +x_0=155000 '
  + '+y_0=463000 +ellps=bessel +towgs84=565.417,50.3319,465.552,-0.398957,0.343988,-1.8774,4.0725 '
  + '+units=m +no_defs');

/** [lon, lat] naar [x, y] in RD (EPSG:28992), op centimeters afgerond. */
export function naarRd([lon, lat]) {
  const [x, y] = proj4('EPSG:4326', 'EPSG:28992', [lon, lat]);
  return [Math.round(x * 100) / 100, Math.round(y * 100) / 100];
}

/** [x, y] in RD naar [lon, lat]. */
export function vanRd([x, y]) {
  return proj4('EPSG:28992', 'EPSG:4326', [x, y]);
}

function inRing([px, py], ring) {
  let binnen = false;
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
    const [xi, yi] = ring[i];
    const [xj, yj] = ring[j];
    if ((yi > py) !== (yj > py) && px < ((xj - xi) * (py - yi)) / (yj - yi) + xi) binnen = !binnen;
  }
  return binnen;
}

function inPolygon(p, rings) {
  if (!inRing(p, rings[0])) return false;
  for (let k = 1; k < rings.length; k++) if (inRing(p, rings[k])) return false;
  return true;
}

/** Ligt punt [lon, lat] in een Polygon of MultiPolygon? */
export function puntInVlak(p, geometry) {
  if (!geometry) return false;
  if (geometry.type === 'Polygon') return inPolygon(p, geometry.coordinates);
  if (geometry.type === 'MultiPolygon') return geometry.coordinates.some((rings) => inPolygon(p, rings));
  return false;
}

function bbox(geometry) {
  let [a, b, c, d] = [Infinity, Infinity, -Infinity, -Infinity];
  const polys = geometry.type === 'Polygon' ? [geometry.coordinates] : geometry.coordinates;
  for (const rings of polys) for (const [x, y] of rings[0]) {
    if (x < a) a = x; if (y < b) b = y; if (x > c) c = x; if (y > d) d = y;
  }
  return [a, b, c, d];
}

/** Voeg aan elk object een bbox toe, zodat zoeken op een punt snel gaat. Eenmalig na laden. */
export function indexeer(featureCollection) {
  for (const f of featureCollection.features) f.bbox ??= bbox(f.geometry);
  return featureCollection;
}

/** Alle objecten uit een FeatureCollection die het punt bevatten. */
export function objectenOpPunt(featureCollection, p) {
  const [x, y] = p;
  return featureCollection.features.filter((f) => {
    const [a, b, c, d] = f.bbox ?? bbox(f.geometry);
    return x >= a && x <= c && y >= b && y <= d && puntInVlak(p, f.geometry);
  });
}

function afstandTotLijnstuk(x, y, [ax, ay], [bx, by]) {
  const dx = bx - ax, dy = by - ay;
  const t = dx || dy ? Math.max(0, Math.min(1, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy))) : 0;
  return Math.hypot(x - (ax + t * dx), y - (ay + t * dy));
}

/**
 * Kleinste afstand in meters (RD) van punt [lon, lat] tot de objecten in de FeatureCollection die
 * aan het filter voldoen; 0 als het punt erin ligt, Infinity als er geen zijn. Meetkunde, geen recht.
 */
export function afstandTot(fc, p, filter) {
  const [x, y] = naarRd(p);
  let min = Infinity;
  for (const f of fc.features) {
    if (!filter(f.properties)) continue;
    if (puntInVlak(p, f.geometry)) return 0;
    const polys = f.geometry.type === 'Polygon' ? [f.geometry.coordinates] : f.geometry.coordinates;
    for (const rings of polys) {
      for (const ring of rings) {
        const r = ring.map(naarRd);
        for (let i = 1; i < r.length; i++) min = Math.min(min, afstandTotLijnstuk(x, y, r[i - 1], r[i]));
      }
    }
  }
  return min;
}
