// RD (EPSG:28992) naar buiten, WGS84 voor Leaflet. Punten als [x, y] in RD en [lon, lat] in WGS84.
import proj4 from 'proj4';

proj4.defs('EPSG:28992', '+proj=sterea +lat_0=52.15616055555555 +lon_0=5.38763888888889 +k=0.9999079 +x_0=155000 +y_0=463000 +ellps=bessel +towgs84=565.4171,50.3319,465.5524,1.9342,-1.6677,9.1019,4.0725 +units=m +no_defs');

/** [lon, lat] naar [x, y] in RD. */
export const naarRd = ([lon, lat]) => proj4('EPSG:4326', 'EPSG:28992', [lon, lat]);
/** [x, y] in RD naar [lon, lat]. */
export const vanRd = ([x, y]) => proj4('EPSG:28992', 'EPSG:4326', [x, y]);
/** Voor koppen en de deellink: "166117, 447996". */
export const rdTekst = ([x, y]) => `${Math.round(x)}, ${Math.round(y)}`;
