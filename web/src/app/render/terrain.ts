import type { MapPayload } from '../worker/protocol';

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
    highlands: [[206, 214, 220], [226, 231, 235]],
    rock: [150, 157, 166],
    snow: [250, 251, 253],
    snowStart: 0.25,
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

export type TerrainView = 'terrain' | 'biomes';

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
  const light = Math.min(1.15, Math.max(0.85, 1 + (below - above) * 0.012));
  out[0] *= light;
  out[1] *= light;
  out[2] *= light;
  if (contours) {
    const band = contourBand(shade[i]);
    const edge =
      (terrain[i + 1] === Terrain.Ocean && contourBand(shade[i + 1]) !== band) ||
      (terrain[i + w] === Terrain.Ocean && contourBand(shade[i + w]) !== band);
    if (edge) mixInto(out, CONTOUR, 0.22);
  }
}

/** Kolor kafla w danym biomie. Wynik trafia do `out` (bez alokacji w pętli). */
function biomeColor(out: number[], t: number, k: number, biome: number, view: TerrainView): void {
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
}

export function paintTerrain(
  map: MapPayload,
  view: TerrainView = 'terrain',
  contours = true,
): Uint8ClampedArray<ArrayBuffer> {
  const { width: w, height: h, terrain, shade, biome, biomeOther, biomeMix } = map;
  const out = new Uint8ClampedArray(w * h * 4);
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
    biomeColor(ca, t, k, biome[i], view);
    // Strefa przejścia: kolor mieszany z drugim biomem według jego udziału w kaflu.
    const mix = biomeMix[i] / 256;
    if (mix > 0) {
      biomeColor(cb, t, k, biomeOther[i], view);
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

/** Proste cieniowanie rzeźby: światło z lewego górnego rogu. */
function hillshade(terrain: Uint8Array, shade: Uint8Array, w: number, h: number, i: number): number {
  const x = i % w;
  const y = (i - x) / w;
  if (x === 0 || y === 0 || x + 1 >= w || y + 1 >= h) return 1;
  const a = terrain[i - w - 1] >= Terrain.Plains ? shade[i - w - 1] : shade[i];
  const b = terrain[i + w + 1] >= Terrain.Plains ? shade[i + w + 1] : shade[i];
  return Math.min(1.3, Math.max(0.7, 1 + (a - b) * 0.02));
}
