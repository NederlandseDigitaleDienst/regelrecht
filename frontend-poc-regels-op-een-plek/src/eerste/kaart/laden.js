// De kaartdata uit de bron: per plan zijn vlakken (GeoJSON) en het zoekgebied.
import { json } from '../../bron.js';
import { indexeer } from './geo.js';

export async function laadKaartdata() {
  const index = json('data/kaart/plannen.json');
  const zoekgebied = json('data/kaart/zoekgebied.geojson');
  const lagen = new Map(index.plannen.map((p) => [p.id, indexeer(json(`data/${p.bestand}`))]));
  return { index, lagen, zoekgebied };
}
