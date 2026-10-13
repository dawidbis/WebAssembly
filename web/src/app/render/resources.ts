import type { MapPayload } from '../worker/protocol';

/** Zasób prowincji na mapie zasobów i w ramce prowincji. */
export interface ResourceInfo {
  name: string;
  /** Kolor na mapie zasobów (hex). */
  color: string;
}

/**
 * Zasoby w kolejności numerów `Province.resource` – jak `RESOURCE_NAMES` w
 * `crates/mapgen/src/resource_table.rs` (generuje go `tools/resources/chances.mjs`; zmieniaj oba).
 * Kolory grupami: uprawy złoto-zielone, hodowla brązowa, las zielony, energia ciemna, metale
 * szaro-niebieskie i rdzawe, przemysł jaskrawy. Każdy kolor odbiega od gór, morza i lodu na mapie
 * (`POLITICAL_*`) i od pozostałych o ΔE ≥ 15 (poza bawełną i lodem – bawełny nie ma w klimacie polarnym).
 */
export const RESOURCES: readonly ResourceInfo[] = [
  { name: 'Zboża', color: '#e8c547' },
  { name: 'Ryż', color: '#dfe6a6' },
  { name: 'Bawełna', color: '#f6f3ea' },
  { name: 'Uprawy plantacyjne', color: '#d9506e' },
  { name: 'Kauczuk', color: '#5d8a2c' },
  { name: 'Bydło', color: '#9c4f2c' },
  { name: 'Skóry', color: '#d1a477' },
  { name: 'Futra', color: '#8e6a7e' },
  { name: 'Drewno', color: '#2d6b43' },
  { name: 'Celuloza', color: '#8fd0a4' },
  { name: 'Węgiel kamienny', color: '#242424' },
  { name: 'Węgiel brunatny', color: '#5e4431' },
  { name: 'Ropa naftowa', color: '#3b2a5c' },
  { name: 'Uran', color: '#7dff3a' },
  { name: 'Rudy żelaza', color: '#b2372a' },
  { name: 'Mangan', color: '#ee93c6' },
  { name: 'Chrom', color: '#bdb6d6' },
  { name: 'Nikiel', color: '#7a9a8c' },
  { name: 'Wolfram', color: '#5576b0' },
  { name: 'Molibden', color: '#25408c' },
  { name: 'Miedź', color: '#d4772f' },
  { name: 'Ołów', color: '#51565f' },
  { name: 'Cynk', color: '#3fa4a8' },
  { name: 'Boksyt', color: '#eaa08a' },
  { name: 'Nawozy mineralne', color: '#86d4f2' },
  { name: 'Materiały budowlane / Cement', color: '#ddd0ad' },
  { name: 'Materiały wybuchowe / Chemia', color: '#b52fc0' },
];

/** Grupy zasobów w legendzie (numery zasobów). */
export const RESOURCE_GROUPS: readonly { name: string; resources: readonly number[] }[] = [
  { name: 'Rolnictwo', resources: [0, 1, 2, 3, 4] },
  { name: 'Hodowla', resources: [5, 6, 7] },
  { name: 'Las', resources: [8, 9] },
  { name: 'Energia', resources: [10, 11, 12, 13] },
  { name: 'Metale', resources: [14, 15, 16, 17, 18, 19, 20, 21, 22, 23] },
  { name: 'Przemysł', resources: [24, 25, 26] },
];

/** Ląd prowincji, zanim faza 3 (`polish`) przydzieli zasoby. */
export const RESOURCE_PENDING = [118, 124, 116] as const;

const rgb = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
const RESOURCE_RGB = RESOURCES.map((r) => rgb(r.color));

/** Kolory prowincji na mapie zasobów (RGB na prowincję, indeks `id - 1`); null przed fazą 3. */
export function resourceColors(map: MapPayload): Float32Array | null {
  if (!map.polished) return null;
  const out = new Float32Array(map.provinces.length * 3);
  map.provinces.forEach((p, k) => out.set(RESOURCE_RGB[p.resource] ?? RESOURCE_PENDING, k * 3));
  return out;
}
