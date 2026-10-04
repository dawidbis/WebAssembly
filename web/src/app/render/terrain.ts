import type { MapPayload } from '../worker/protocol';
import { POLITICAL_BORDER, POLITICAL_ICE, POLITICAL_LAKE, POLITICAL_MOUNTAIN, POLITICAL_SEA, politicalColors, provinceBorder } from './provinces';

/** Typy kafli – muszą zgadzać się z `game_mapgen::Terrain`. */
export const Terrain = {
  Ocean: 0,
  Lake: 1,
  River: 2,
  Plains: 3,
  Highlands: 4,
  Mountains: 5,
} as const;

/** Typy klimatu – muszą zgadzać się z `game_mapgen::BiomeType`. */
export const BiomeType = {
  Tropical: 0,
  Dry: 1,
  Temperate: 2,
  Continental: 3,
  Polar: 4,
} as const;

/** Nazwy typów klimatu w kolejności `BiomeType`. */
export const BIOME_TYPES: readonly string[] = ['Tropikalny', 'Suchy', 'Umiarkowany', 'Kontynentalny', 'Polarny'];

/** Rodzaje biomów – muszą zgadzać się z `game_mapgen::Biome`. */
export const Biome = {
  Rainforest: 0,
  Savanna: 1,
  Desert: 2,
  Steppe: 3,
  Mediterranean: 4,
  Subtropical: 5,
  Oceanic: 6,
  HotSummer: 7,
  WarmSummer: 8,
  Boreal: 9,
  Taiga: 10,
  Tundra: 11,
  IceSheet: 12,
} as const;

const KINDS = 13;

/**
 * Wagi rodzajów (suma 1) w kaflu `i` z warstw typów – dokładnie jak `kind_weights` w Ruście
 * (`crates/mapgen/src/biome.rs`). Wynik trafia do `out` (bez alokacji w pętli).
 */
export function kindWeights(out: Float32Array, layers: Uint8Array, mix: Uint8Array, i: number): void {
  out.fill(0);
  const t = mix[i] / 255;
  addLayer(out, layers, 6 * i, 1 - t);
  addLayer(out, layers, 6 * i + 3, t);
}

function addLayer(w: Float32Array, l: Uint8Array, o: number, share: number): void {
  if (share <= 0) return;
  const a = l[o + 1] / 255;
  const b = l[o + 2] / 255;
  switch (l[o]) {
    case BiomeType.Tropical:
      w[Biome.Rainforest] += share * (1 - a);
      w[Biome.Savanna] += share * a;
      break;
    case BiomeType.Dry:
      w[Biome.Steppe] += share * (1 - a);
      w[Biome.Desert] += share * a;
      break;
    case BiomeType.Temperate:
      w[Biome.Oceanic] += share * a;
      w[Biome.Mediterranean] += share * (1 - a) * b;
      w[Biome.Subtropical] += share * (1 - a) * (1 - b);
      break;
    case BiomeType.Continental:
      w[Biome.Boreal] += share * a;
      w[Biome.WarmSummer] += share * Math.max(0, b - a);
      w[Biome.HotSummer] += share * (1 - Math.max(a, b));
      break;
    default:
      w[Biome.IceSheet] += share * a;
      w[Biome.Tundra] += share * Math.max(0, b - a);
      w[Biome.Taiga] += share * (1 - Math.max(a, b));
  }
}

type Rgb = readonly [number, number, number];

interface Palette {
  plains: readonly [Rgb, Rgb];
  highlands: readonly [Rgb, Rgb];
  rock: Rgb;
  snow: Rgb;
  /** Od jakiej wysokości (0..1) góry bieleją i na jakiej są już całe w śniegu/lodzie. */
  snowStart: number;
  snowFull: number;
  lake: Rgb;
  river: Rgb;
}

/**
 * Ta sama paleta co w CLI `mapgen` (crates/mapgen/src/bin/mapgen.rs), w kolejności `Biome`:
 * las deszczowy, sawanna, pustynia, step, śródziemnomorski, subtropikalny, oceaniczny,
 * gorące lato, ciepłe lato, borealny, tajga (polarna), tundra, lądolód.
 */
const PALETTES: readonly Palette[] = [
  {
    plains: [[40, 108, 50], [64, 130, 58]],
    highlands: [[78, 118, 60], [98, 112, 68]],
    rock: [96, 106, 92],
    snow: [214, 220, 212],
    snowStart: 0.8,
    snowFull: 1.0,
    lake: [48, 110, 120],
    river: [58, 122, 138],
  },
  {
    plains: [[188, 174, 94], [204, 188, 112]],
    highlands: [[184, 156, 98], [160, 132, 88]],
    rock: [146, 120, 96],
    snow: [232, 224, 210],
    snowStart: 0.8,
    snowFull: 1.0,
    lake: [60, 140, 150],
    river: [70, 150, 170],
  },
  {
    plains: [[222, 196, 138], [238, 214, 162]],
    highlands: [[210, 162, 104], [184, 130, 88]],
    rock: [158, 114, 84],
    snow: [228, 204, 172],
    snowStart: 0.8,
    snowFull: 1.0,
    lake: [58, 150, 168],
    river: [70, 156, 176],
  },
  {
    plains: [[166, 170, 106], [186, 184, 124]],
    highlands: [[172, 158, 108], [150, 134, 96]],
    rock: [140, 124, 108],
    snow: [234, 230, 222],
    snowStart: 0.7,
    snowFull: 1.0,
    lake: [72, 138, 168],
    river: [80, 146, 182],
  },
  {
    plains: [[142, 154, 84], [170, 170, 104]],
    highlands: [[150, 140, 90], [136, 122, 86]],
    rock: [150, 132, 112],
    snow: [236, 232, 224],
    snowStart: 0.65,
    snowFull: 1.0,
    lake: [54, 132, 176],
    river: [66, 142, 190],
  },
  {
    plains: [[88, 146, 66], [120, 162, 84]],
    highlands: [[80, 126, 62], [100, 116, 72]],
    rock: [122, 116, 104],
    snow: [236, 236, 230],
    snowStart: 0.65,
    snowFull: 1.0,
    lake: [56, 128, 170],
    river: [66, 138, 186],
  },
  {
    plains: [[104, 150, 72], [150, 170, 96]],
    highlands: [[88, 128, 64], [110, 118, 76]],
    rock: [128, 118, 108],
    snow: [238, 236, 230],
    snowStart: 0.55,
    snowFull: 1.0,
    lake: [63, 134, 184],
    river: [74, 144, 196],
  },
  {
    plains: [[124, 154, 78], [160, 172, 100]],
    highlands: [[110, 132, 70], [128, 124, 82]],
    rock: [130, 120, 108],
    snow: [240, 238, 234],
    snowStart: 0.55,
    snowFull: 1.0,
    lake: [62, 132, 178],
    river: [72, 142, 192],
  },
  {
    plains: [[104, 144, 82], [144, 162, 106]],
    highlands: [[94, 124, 74], [114, 118, 86]],
    rock: [124, 120, 116],
    snow: [242, 242, 240],
    snowStart: 0.5,
    snowFull: 1.0,
    lake: [70, 136, 180],
    river: [80, 146, 192],
  },
  {
    plains: [[58, 104, 70], [84, 124, 86]],
    highlands: [[66, 100, 74], [90, 112, 90]],
    rock: [108, 112, 112],
    snow: [246, 247, 248],
    snowStart: 0.5,
    snowFull: 1.0,
    lake: [70, 130, 160],
    river: [80, 140, 176],
  },
  {
    plains: [[92, 123, 90], [117, 142, 108]],
    highlands: [[95, 118, 94], [118, 132, 114]],
    rock: [108, 112, 112],
    snow: [236, 244, 250],
    snowStart: 0.48,
    snowFull: 0.56,
    lake: [88, 144, 173],
    river: [93, 148, 184],
  },
  {
    plains: [[156, 158, 128], [178, 176, 150]],
    highlands: [[150, 150, 132], [170, 170, 160]],
    rock: [118, 118, 122],
    snow: [236, 244, 250],
    snowStart: 0.41,
    snowFull: 0.49,
    lake: [206, 222, 232],
    river: [198, 216, 228],
  },
  {
    plains: [[222, 229, 233], [238, 242, 245]],
    highlands: [[176, 190, 204], [196, 208, 220]],
    rock: [196, 208, 222],
    snow: [250, 251, 253],
    snowStart: 0.0,
    snowFull: 1.0,
    lake: [206, 222, 232],
    river: [198, 216, 228],
  },
];

/** Nazwy i płaskie kolory biomów (widok „mapa biomów”, legenda w panelu) – w kolejności `Biome`. */
export const BIOMES: readonly { name: string; color: Rgb }[] = [
  { name: 'Tropikalny – las deszczowy', color: [40, 118, 56] },
  { name: 'Tropikalny – sawanna', color: [198, 178, 92] },
  { name: 'Suchy – pustynia', color: [230, 204, 146] },
  { name: 'Suchy – step', color: [180, 178, 112] },
  { name: 'Umiarkowany – śródziemnomorski', color: [156, 164, 82] },
  { name: 'Umiarkowany – subtropikalny', color: [82, 150, 66] },
  { name: 'Umiarkowany – oceaniczny', color: [112, 164, 100] },
  { name: 'Kontynentalny – gorące lato', color: [130, 160, 112] },
  { name: 'Kontynentalny – ciepłe lato', color: [100, 140, 112] },
  { name: 'Kontynentalny – borealny', color: [52, 104, 80] },
  { name: 'Polarny – tajga', color: [96, 124, 112] },
  { name: 'Polarny – tundra', color: [170, 172, 148] },
  { name: 'Polarny – lądolód', color: [228, 238, 244] },
];

/**
 * Pary typów klimatu – indeks = numer bitu w `MapGenParams.biomePairs`.
 * Kolejność musi zgadzać się z `game_mapgen::BIOME_PAIRS`.
 */
export const BIOME_PAIRS: readonly (readonly [number, number])[] = [
  [BiomeType.Tropical, BiomeType.Dry],
  [BiomeType.Tropical, BiomeType.Temperate],
  [BiomeType.Tropical, BiomeType.Continental],
  [BiomeType.Tropical, BiomeType.Polar],
  [BiomeType.Dry, BiomeType.Temperate],
  [BiomeType.Dry, BiomeType.Continental],
  [BiomeType.Dry, BiomeType.Polar],
  [BiomeType.Temperate, BiomeType.Continental],
  [BiomeType.Temperate, BiomeType.Polar],
  [BiomeType.Continental, BiomeType.Polar],
];

/** Rodzaje każdego typu w kolejności `Biome` (bit j maski wariantu = j-ty rodzaj) – jak `BiomeType::kinds`. */
export const TYPE_KINDS: readonly (readonly number[])[] = [
  [Biome.Rainforest, Biome.Savanna],
  [Biome.Desert, Biome.Steppe],
  [Biome.Mediterranean, Biome.Subtropical, Biome.Oceanic],
  [Biome.HotSummer, Biome.WarmSummer, Biome.Boreal],
  [Biome.Taiga, Biome.Tundra, Biome.IceSheet],
];

/**
 * Warianty typów w kolejności wag `MapGenParams.biomeVariants` (jak `variant_index` w Ruście):
 * typy po kolei, w typie maski 1..2^n − 1. `kinds` – rodzaje wariantu.
 */
export const VARIANTS: readonly { type: number; kinds: number[] }[] = TYPE_KINDS.flatMap((kinds, type) =>
  Array.from({ length: (1 << kinds.length) - 1 }, (_, m) => ({ type, kinds: kinds.filter((_, j) => ((m + 1) >> j) & 1) })),
);

/** Styl mapy. `political` = same prowincje (bez rzeźby, lasów i rzek), jak `--view political` w CLI. */
export type TerrainView = 'terrain' | 'biomes' | 'political';

/** Kolory koron drzew w kolejności `Biome` – jak `CANOPY` w CLI. */
const CANOPY: readonly Rgb[] = [
  [22, 78, 34],
  [98, 116, 52],
  [58, 112, 52],
  [82, 112, 54],
  [72, 98, 52],
  [34, 88, 40],
  [52, 98, 44],
  [60, 100, 46],
  [48, 92, 52],
  [30, 70, 48],
  [38, 80, 56],
  [92, 110, 80],
  [200, 210, 215],
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

/** Ile kafli koron jest przyprószonych śniegiem, w kolejności `Biome` (krzewy tundry) – jak `CANOPY_SNOW` w CLI. */
const CANOPY_SNOW = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0.3, 0];
const CANOPY_SNOW_COLOR: Rgb = [226, 234, 240];

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
/** Las w kaflu: gęstość 0..1 i losy ziarna koron (jak drugi i trzeci element `grain` w CLI). */
interface TileForest {
  forest: number;
  grain: number;
  roll: number;
  snow: number;
}

const NO_FOREST: TileForest = { forest: 0, grain: 1, roll: 0, snow: 1 };

function biomeColor(out: number[], t: number, k: number, biome: number, view: TerrainView, wood: TileForest = NO_FOREST): void {
  groundColor(out, t, k, biome, view);
  if (wood.forest > 0 && t >= Terrain.Plains) addForest(out, biome, view, wood);
}

/** Kolor gruntu, wody albo gór w danym rodzaju biomu (bez lasu). */
function groundColor(out: number[], t: number, k: number, biome: number, view: TerrainView): void {
  const p = PALETTES[biome] ?? PALETTES[Biome.Oceanic];
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
    a = b = BIOMES[biome]?.color ?? BIOMES[Biome.Oceanic].color;
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
    f = Math.min(1, Math.max(0, (k - p.snowStart) / (p.snowFull - p.snowStart)));
  }
  out[0] = a[0] + (b[0] - a[0]) * f;
  out[1] = a[1] + (b[1] - a[1]) * f;
  out[2] = a[2] + (b[2] - a[2]) * f;
}

/** Korony drzew nałożone na grunt (na płaskiej mapie biomów – tylko przyciemnienie). */
function addForest(out: number[], biome: number, view: TerrainView, { forest, grain: grainValue, roll, snow }: TileForest): void {
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
  const c = CANOPY[biome] ?? CANOPY[Biome.Oceanic];
  let r = c[0] * grainValue;
  let g = c[1] * grainValue;
  let bl = c[2] * grainValue;
  // Tajga i tundra przyprószone śniegiem: lekko rozjaśniona, a część koron z białą plamką.
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

/** Lód morski – jak `SEA_ICE` w CLI. */
const SEA_ICE: Rgb = [226, 234, 240];

/**
 * Maska lodu (1 = lód): lodowiec z generatora (`glacier` – lądolód i kieszenie lądu zamknięte
 * w lodzie), wody śródlądowe, na których lądolód ma co najmniej połowę wagi, oraz lód morski.
 * Do wyraźnej krawędzi lądolodu i jego cieniowania (jak `ice_mask` w CLI).
 */
export function iceMask(map: MapPayload): Uint8Array {
  const { width: w, height: h, terrain, biomeLayers, biomeMix, seaIce, glacier } = map;
  const mask = new Uint8Array(w * h);
  const weights = new Float32Array(KINDS);
  for (let i = 0; i < w * h; i++) {
    if (terrain[i] === Terrain.Ocean) {
      mask[i] = seaIce[i];
    } else if (terrain[i] >= Terrain.Plains) {
      mask[i] = glacier[i];
    } else {
      kindWeights(weights, biomeLayers, biomeMix, i);
      mask[i] = weights[Biome.IceSheet] >= 0.5 ? 1 : 0;
    }
  }
  return mask;
}

/**
 * Lądolód „odstający od lądu” przy świetle z lewego górnego rogu: oświetlona krawędź od strony
 * światła, ciemniejsza ściana klifu od prawej i od dołu, cień rzucany w prawo w dół na ląd
 * i wodę (jak `ice_shade` w CLI). Zwraca mnożnik jasności.
 */
function iceShade(mask: Uint8Array, w: number, h: number, i: number): number {
  const x = i % w;
  const y = (i - x) / w;
  const at = (dx: number, dy: number): number => {
    const nx = x + dx;
    const ny = y + dy;
    return nx >= 0 && ny >= 0 && nx < w && ny < h ? mask[ny * w + nx] : 0;
  };
  if (mask[i]) {
    if (!at(1, 0) || !at(0, 1) || !at(1, 1)) return 0.8;
    if (!at(-1, 0) || !at(0, -1)) return 1.08;
    return 1;
  }
  if (at(-1, -1) || at(-1, 0) || at(0, -1)) return 0.72;
  if (at(-2, -2) || at(-2, -1) || at(-1, -2)) return 0.86;
  return 1;
}

/** Wyraźny lądolód: kafel z maską lodu jest samym lądolodem, inne – bez lądolodu. */
function sharpenIce(weights: Float32Array, ice: boolean): void {
  if (ice) {
    weights.fill(0);
    weights[Biome.IceSheet] = 1;
    return;
  }
  const rest = 1 - weights[Biome.IceSheet];
  weights[Biome.IceSheet] = 0;
  if (rest <= 0) {
    weights[Biome.Tundra] = 1;
    return;
  }
  for (let b = 0; b < KINDS; b++) weights[b] /= rest;
}

export function paintTerrain(
  map: MapPayload,
  view: TerrainView = 'terrain',
  contours = true,
): Uint8ClampedArray<ArrayBuffer> {
  const { width: w, height: h, terrain, shade, biomeLayers, biomeMix, forest } = map;
  const out = new Uint8ClampedArray(w * h * 4);
  if (view === 'political') return paintPolitical(map, out);
  const ca = [0, 0, 0];
  const cb = [0, 0, 0];
  const weights = new Float32Array(KINDS);
  // Lodowiec (`glacier`): wyraźna krawędź lądolodu z cieniowaniem (lądolód odstający od lądu).
  const ice = map.params.glacier ? iceMask(map) : null;
  const shadeIce = !!ice && view === 'terrain';

  const wood: TileForest = { forest: 0, grain: 1, roll: 0, snow: 1 };

  for (let i = 0; i < w * h; i++) {
    const t = terrain[i];
    if (t === Terrain.Ocean) {
      oceanTile(ca, map, i, contours);
    } else {
      forestAt(wood, forest[i] / 255, i % w, Math.floor(i / w));
      // Płynne przejścia: kolory rodzajów mieszane według ich wag w kaflu (jak w CLI).
      kindWeights(weights, biomeLayers, biomeMix, i);
      if (ice) sharpenIce(weights, ice[i] === 1);
      blendKinds(ca, cb, weights, t, shade[i] / 255, view, wood);
    }
    let light = t >= Terrain.Plains ? hillshade(terrain, shade, w, h, i) : 1;
    if (shadeIce) light *= iceShade(ice, w, h, i);
    const o = i * 4;
    out[o] = ca[0] * light;
    out[o + 1] = ca[1] * light;
    out[o + 2] = ca[2] * light;
    out[o + 3] = 255;
  }
  return out;
}

/** Kafel oceanu: lód morski albo woda z głębią, linią brzegu i izobatami. */
function oceanTile(out: number[], map: MapPayload, i: number, contours: boolean): void {
  if (!map.seaIce[i]) {
    oceanColor(out, map, i, contours);
    return;
  }
  const x = i % map.width;
  const v = 0.96 + 0.06 * tileHash(x + 11, (i - x) / map.width + 5);
  out[0] = SEA_ICE[0] * v;
  out[1] = SEA_ICE[1] * v;
  out[2] = SEA_ICE[2] * v;
}

/** Las kafla (gęstość i losy ziarna koron) – zapis do `wood` bez alokacji. */
function forestAt(wood: TileForest, density: number, x: number, y: number): void {
  wood.forest = density;
  const any = density > 0;
  wood.grain = any ? grain(x, y) : 1;
  wood.roll = any ? treeRoll(x, y) : 0;
  wood.snow = any ? snowRoll(x, y) : 1;
}

/** Kolor kafla lądu lub wód śródlądowych: kolory rodzajów ważone ich udziałem. */
function blendKinds(out: number[], tmp: number[], weights: Float32Array, t: number, k: number, view: TerrainView, wood: TileForest): void {
  out[0] = out[1] = out[2] = 0;
  for (let b = 0; b < KINDS; b++) {
    const wb = weights[b];
    if (wb <= 0) continue;
    biomeColor(tmp, t, k, b, view, wood);
    out[0] += tmp[0] * wb;
    out[1] += tmp[1] * wb;
    out[2] += tmp[2] * wb;
  }
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
    } else if (terrain[i] === Terrain.Lake || terrain[i] === Terrain.River) {
      c = POLITICAL_LAKE;
    } else if (p > 0) {
      c = colors;
      k = (p - 1) * 3;
    } else if (terrain[i] !== Terrain.Mountains && map.glacier[i]) {
      c = POLITICAL_ICE;
    } else if (terrain[i] >= Terrain.Plains) {
      c = POLITICAL_MOUNTAIN; // góry i enklawy – ląd niczyj
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
