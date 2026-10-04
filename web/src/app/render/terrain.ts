import type { MapPayload } from '../worker/protocol';
import { POLITICAL_BORDER, POLITICAL_LAKE, POLITICAL_MOUNTAIN, POLITICAL_SEA, politicalColors, provinceBorder } from './provinces';

/** Typy kafli – muszą zgadzać się z `game_mapgen::Terrain`. */
export const Terrain = {
  Ocean: 0,
  Lake: 1,
  River: 2,
  Plains: 3,
  Highlands: 4,
  Mountains: 5,
} as const;

/** Biomy – muszą zgadzać się z `game_mapgen::Biome`. */
export const Biome = {
  Temperate: 0,
  Desert: 1,
  Cold: 2,
  Humid: 3,
  Steppe: 4,
} as const;

type Rgb = readonly [number, number, number];

interface Palette {
  plains: readonly [Rgb, Rgb];
  highlands: readonly [Rgb, Rgb];
  rock: Rgb;
  snow: Rgb;
  /** Od jakiej wysokości (0..1) góry bieleją. */
  snowStart: number;
  lake: Rgb;
  river: Rgb;
}

/** Ta sama paleta co w CLI `mapgen` (crates/mapgen/src/bin/mapgen.rs), w kolejności `Biome`. */
const PALETTES: readonly Palette[] = [
  {
    plains: [[104, 150, 72], [150, 170, 96]],
    highlands: [[88, 128, 64], [110, 118, 76]],
    rock: [128, 118, 108],
    snow: [238, 236, 230],
    snowStart: 0.55,
    lake: [63, 134, 184],
    river: [74, 144, 196],
  },
  {
    plains: [[222, 196, 138], [238, 214, 162]],
    highlands: [[210, 162, 104], [184, 130, 88]],
    rock: [158, 114, 84],
    snow: [228, 204, 172],
    snowStart: 0.8,
    lake: [58, 150, 168],
    river: [70, 156, 176],
  },
  {
    plains: [[222, 229, 233], [238, 242, 245]],
    highlands: [[176, 190, 204], [196, 208, 220]],
    rock: [104, 112, 124],
    snow: [250, 251, 253],
    snowStart: 0.45,
    lake: [148, 188, 210],
    river: [126, 174, 206],
  },
  {
    plains: [[40, 108, 50], [64, 130, 58]],
    highlands: [[78, 118, 60], [98, 112, 68]],
    rock: [96, 106, 92],
    snow: [214, 220, 212],
    snowStart: 0.8,
    lake: [48, 110, 120],
    river: [58, 122, 138],
  },
  {
    plains: [[172, 170, 100], [190, 180, 114]],
    highlands: [[180, 156, 104], [158, 130, 90]],
    rock: [140, 124, 108],
    snow: [234, 230, 222],
    snowStart: 0.7,
    lake: [72, 138, 168],
    river: [80, 146, 182],
  },
];

/** Nazwy i płaskie kolory biomów (widok „mapa biomów”, legenda w panelu) – w kolejności `Biome`. */
export const BIOMES: readonly { name: string; color: Rgb }[] = [
  { name: 'Umiarkowany', color: [106, 154, 72] },
  { name: 'Pustynny', color: [224, 196, 138] },
  { name: 'Zimny', color: [216, 228, 234] },
  { name: 'Wilgotny', color: [47, 122, 60] },
  { name: 'Step', color: [184, 174, 102] },
];

/**
 * Pary biomów – indeks = numer bitu w `MapGenParams.biomePairs`.
 * Kolejność musi zgadzać się z `game_mapgen::BIOME_PAIRS`.
 */
export const BIOME_PAIRS: readonly (readonly [number, number])[] = [
  [Biome.Temperate, Biome.Desert],
  [Biome.Temperate, Biome.Cold],
  [Biome.Temperate, Biome.Humid],
  [Biome.Temperate, Biome.Steppe],
  [Biome.Desert, Biome.Cold],
  [Biome.Desert, Biome.Humid],
  [Biome.Desert, Biome.Steppe],
  [Biome.Cold, Biome.Humid],
  [Biome.Cold, Biome.Steppe],
  [Biome.Humid, Biome.Steppe],
];

/** Styl mapy. `political` = same prowincje (bez rzeźby, lasów i rzek), jak `--view political` w CLI. */
export type TerrainView = 'terrain' | 'biomes' | 'fertility' | 'political';

/** Kolory koron drzew w kolejności `Biome`: liściasty, oazy (palmy), tajga, dżungla, zagajniki – jak `CANOPY` w CLI. */
const CANOPY: readonly Rgb[] = [
  [52, 98, 44],
  [58, 112, 52],
  [62, 90, 80],
  [22, 78, 34],
  [82, 112, 54],
];

function tileHash(x: number, y: number): number {
  let h = Math.imul(x, 0x9e3779b1) ^ Math.imul(y, 0x85ebca77);
  h = Math.imul(h ^ (h >>> 15), 0x2c1b3c6d);
  return ((h ^ (h >>> 12)) >>> 0) / 4294967295;
}

/** Ziarno koron drzew: jasność per kafel i per blok 2×2 (jak `grain` w CLI). */
function grain(x: number, y: number): number {
  return 0.72 + 0.34 * tileHash(x, y) + 0.2 * tileHash(x >> 1, y >> 1);
}

/** Los kafla na skraju lasu (jak drugi element `grain` w CLI). */
function treeRoll(x: number, y: number): number {
  return tileHash(x + 17, y + 31);
}

/** Los śniegu na koronie (jak trzeci element `grain` w CLI). */
function snowRoll(x: number, y: number): number {
  return tileHash(x + 53, y + 97);
}

/** Ile kafli koron jest przyprószonych śniegiem, w kolejności `Biome` (tylko tajga) – jak `CANOPY_SNOW` w CLI. */
const CANOPY_SNOW = [0, 0, 0.42, 0, 0];
const CANOPY_SNOW_COLOR: Rgb = [226, 234, 240];

/** Widok żyzności: od jałowego brązu przez słomkowy do soczystej zieleni (jak `fertility_color` w CLI). */
function fertilityColor(out: number[], f: number): void {
  const k = f / 255;
  const [a, b, t]: [Rgb, Rgb, number] =
    k < 0.5 ? [[120, 96, 70], [196, 180, 96], k * 2] : [[196, 180, 96], [60, 150, 50], k * 2 - 1];
  out[0] = a[0] + (b[0] - a[0]) * t;
  out[1] = a[1] + (b[1] - a[1]) * t;
  out[2] = a[2] + (b[2] - a[2]) * t;
}

/** Kolory oceanu według głębokości 0..1 – te same co `OCEAN_STOPS` w CLI `mapgen`. */
const OCEAN_STOPS: readonly (readonly [number, Rgb])[] = [
  [0, [92, 176, 200]],
  [0.14, [62, 142, 182]],
  [0.3, [34, 92, 142]],
  [0.72, [16, 52, 94]],
  [1, [8, 27, 56]],
];
const COAST_LINE: Rgb = [196, 230, 236];
const CONTOUR: Rgb = [200, 225, 240];
/** Poziomy izobat (głębokość 0..255) – jak `CONTOUR_LEVELS` w CLI. */
const CONTOUR_LEVELS = [30, 70, 120, 175, 225];
/** Krycie izobat – słabe, żeby linie nie były ostre (jak `CONTOUR_OPACITY` w CLI). */
const CONTOUR_OPACITY = 0.08;

function oceanDepthColor(out: number[], k: number): void {
  for (let s = 1; s < OCEAN_STOPS.length; s++) {
    const [k0, c0] = OCEAN_STOPS[s - 1];
    const [k1, c1] = OCEAN_STOPS[s];
    if (k <= k1 || s === OCEAN_STOPS.length - 1) {
      const f = Math.min(1, Math.max(0, (k - k0) / (k1 - k0)));
      out[0] = c0[0] + (c1[0] - c0[0]) * f;
      out[1] = c0[1] + (c1[1] - c0[1]) * f;
      out[2] = c0[2] + (c1[2] - c0[2]) * f;
      return;
    }
  }
}

function mixInto(out: number[], c: Rgb, f: number): void {
  out[0] += (c[0] - out[0]) * f;
  out[1] += (c[1] - out[1]) * f;
  out[2] += (c[2] - out[2]) * f;
}

function contourBand(depth: number): number {
  let n = 0;
  for (const level of CONTOUR_LEVELS) if (depth >= level) n++;
  return n;
}

/** Ocean: kolor głębokości, cieniowanie dna, jasna linia brzegu i izobaty (jak `ocean_color` w CLI). */
function oceanColor(out: number[], map: MapPayload, i: number, contours: boolean): void {
  const { width: w, height: h, terrain, shade } = map;
  oceanDepthColor(out, shade[i] / 255);
  const x = i % w;
  const y = (i - x) / w;
  if (x === 0 || y === 0 || x + 1 >= w || y + 1 >= h) return;
  if (terrain[i - 1] || terrain[i + 1] || terrain[i - w] || terrain[i + w]) {
    mixInto(out, COAST_LINE, 0.55);
    return;
  }
  // Dno oświetlone jak ląd (wysokość = -głębokość), słabiej. Ląd liczy się jako głębokość 0.
  const below = terrain[i + w + 1] === Terrain.Ocean ? shade[i + w + 1] : 0;
  const above = terrain[i - w - 1] === Terrain.Ocean ? shade[i - w - 1] : 0;
  const light = Math.min(1.15, Math.max(0.85, 1 + (above - below) * 0.012));
  out[0] *= light;
  out[1] *= light;
  out[2] *= light;
  if (contours) {
    const band = contourBand(shade[i]);
    const edge =
      (terrain[i + 1] === Terrain.Ocean && contourBand(shade[i + 1]) !== band) ||
      (terrain[i + w] === Terrain.Ocean && contourBand(shade[i + w]) !== band);
    if (edge) mixInto(out, CONTOUR, CONTOUR_OPACITY);
  }
}

/** Kolor kafla w danym biomie. Wynik trafia do `out` (bez alokacji w pętli). */
function biomeColor(
  out: number[],
  t: number,
  k: number,
  biome: number,
  view: TerrainView,
  forest = 0,
  grainValue = 1,
  roll = 0,
  snow = 1,
): void {
  const p = PALETTES[biome] ?? PALETTES[Biome.Temperate];
  let a: Rgb;
  let b: Rgb;
  let f = 0;
  if (t === Terrain.Ocean) {
    oceanDepthColor(out, k);
    return;
  } else if (t === Terrain.Lake) {
    a = b = p.lake;
  } else if (t === Terrain.River) {
    a = b = p.river;
  } else if (view === 'biomes') {
    a = b = BIOMES[biome]?.color ?? BIOMES[Biome.Temperate].color;
  } else if (t === Terrain.Plains) {
    a = p.plains[0];
    b = p.plains[1];
    f = k;
  } else if (t === Terrain.Highlands) {
    a = p.highlands[0];
    b = p.highlands[1];
    f = k;
  } else {
    a = p.rock;
    b = p.snow;
    f = Math.min(1, Math.max(0, (k - p.snowStart) / (1 - p.snowStart)));
  }
  out[0] = a[0] + (b[0] - a[0]) * f;
  out[1] = a[1] + (b[1] - a[1]) * f;
  out[2] = a[2] + (b[2] - a[2]) * f;
  if (forest <= 0 || t < Terrain.Plains) return;
  if (view === 'biomes') {
    // Las na płaskiej mapie biomów: ten sam kolor, tylko ciemniejszy.
    const dark = 1 - 0.25 * forest;
    out[0] *= dark;
    out[1] *= dark;
    out[2] *= dark;
    return;
  }
  // Korony drzew nałożone na grunt. Na skraju (gęstość < 1) las rozpada się na pojedyncze
  // drzewa: kafel jest zadrzewiony, gdy jego los < gęstość (jak `with_forest` w CLI).
  const c = CANOPY[biome] ?? CANOPY[Biome.Temperate];
  let r = c[0] * grainValue;
  let g = c[1] * grainValue;
  let bl = c[2] * grainValue;
  // Tajga przyprószona śniegiem: lekko rozjaśniona, a część koron z białą plamką.
  const snowShare = CANOPY_SNOW[biome] ?? 0;
  if (snowShare > 0) {
    const s = CANOPY_SNOW_COLOR;
    // Udział koloru korony: 0.85 (rozjaśnienie o 0.15), a z plamką jeszcze × 0.4 (dodatkowe 0.6 śniegu).
    const k = snow < snowShare ? 0.85 * 0.4 : 0.85;
    r = s[0] + (r - s[0]) * k;
    g = s[1] + (g - s[1]) * k;
    bl = s[2] + (bl - s[2]) * k;
  }
  const kf = roll < forest ? 0.92 : forest * 0.25;
  out[0] += (r - out[0]) * kf;
  out[1] += (g - out[1]) * kf;
  out[2] += (bl - out[2]) * kf;
}

export function paintTerrain(
  map: MapPayload,
  view: TerrainView = 'terrain',
  contours = true,
): Uint8ClampedArray<ArrayBuffer> {
  const { width: w, height: h, terrain, shade, biome, biomeOther, biomeMix, forest, fertility } = map;
  const out = new Uint8ClampedArray(w * h * 4);
  if (view === 'political') return paintPolitical(map, out);
  const ca = [0, 0, 0];
  const cb = [0, 0, 0];

  for (let i = 0; i < w * h; i++) {
    const t = terrain[i];
    const o = i * 4;
    if (t === Terrain.Ocean) {
      oceanColor(ca, map, i, contours);
      out[o] = ca[0];
      out[o + 1] = ca[1];
      out[o + 2] = ca[2];
      out[o + 3] = 255;
      continue;
    }
    const k = shade[i] / 255;
    const x = i % w;
    const fk = forest[i] / 255;
    const y = (i - x) / w;
    const gr = fk > 0 ? grain(x, y) : 1;
    const roll = fk > 0 ? treeRoll(x, y) : 0;
    const snow = fk > 0 ? snowRoll(x, y) : 1;
    if (view === 'fertility' && t >= Terrain.Plains) {
      fertilityColor(ca, fertility[i]);
      const light = hillshade(terrain, shade, w, h, i);
      out[o] = ca[0] * light;
      out[o + 1] = ca[1] * light;
      out[o + 2] = ca[2] * light;
      out[o + 3] = 255;
      continue;
    }
    biomeColor(ca, t, k, biome[i], view, fk, gr, roll, snow);
    // Strefa przejścia: kolor mieszany z drugim biomem według jego udziału w kaflu.
    const mix = biomeMix[i] / 256;
    if (mix > 0) {
      biomeColor(cb, t, k, biomeOther[i], view, fk, gr, roll, snow);
      ca[0] += (cb[0] - ca[0]) * mix;
      ca[1] += (cb[1] - ca[1]) * mix;
      ca[2] += (cb[2] - ca[2]) * mix;
    }

    const light = t >= Terrain.Plains ? hillshade(terrain, shade, w, h, i) : 1;
    out[o] = ca[0] * light;
    out[o + 1] = ca[1] * light;
    out[o + 2] = ca[2] * light;
    out[o + 3] = 255;
  }
  return out;
}

/** Mapa polityczna: płaskie kolory prowincji z granicami, woda jednolita (jak `--view political` w CLI). */
function paintPolitical(map: MapPayload, out: Uint8ClampedArray<ArrayBuffer>): Uint8ClampedArray<ArrayBuffer> {
  const { width: w, height: h, terrain, province } = map;
  const colors = politicalColors(map);
  for (let i = 0; i < w * h; i++) {
    const o = i * 4;
    const p = province[i];
    let c: ArrayLike<number>;
    let k = 0;
    if (provinceBorder(province, w, h, i)) {
      c = POLITICAL_BORDER;
    } else if (terrain[i] === Terrain.Lake) {
      c = POLITICAL_LAKE;
    } else if (p > 0) {
      c = colors;
      k = (p - 1) * 3;
    } else if (terrain[i] >= Terrain.River) {
      c = POLITICAL_MOUNTAIN;
    } else {
      c = POLITICAL_SEA;
    }
    out[o] = c[k];
    out[o + 1] = c[k + 1];
    out[o + 2] = c[k + 2];
    out[o + 3] = 255;
  }
  return out;
}

/**
 * Proste cieniowanie rzeźby: światło z lewego górnego rogu (konwencja kartograficzna).
 * Zbocze opadające w stronę światła (wyżej w prawo w dół) jest jaśniejsze, odwrócone – ciemniejsze.
 */
function hillshade(terrain: Uint8Array, shade: Uint8Array, w: number, h: number, i: number): number {
  const x = i % w;
  const y = (i - x) / w;
  if (x === 0 || y === 0 || x + 1 >= w || y + 1 >= h) return 1;
  const a = terrain[i - w - 1] >= Terrain.Plains ? shade[i - w - 1] : shade[i];
  const b = terrain[i + w + 1] >= Terrain.Plains ? shade[i + w + 1] : shade[i];
  return Math.min(1.3, Math.max(0.7, 1 + (b - a) * 0.02));
}
