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

/**
 * Grupy zasobów (legenda) i obwódka prowincji na mapie zasobów: kolor i grubość w kaflach.
 * Energia i metale mają jaskrawą, podwójną obwódkę – mają się wyróżniać; reszta ciemną i cienką.
 */
export const RESOURCE_GROUPS: readonly { name: string; resources: readonly number[]; border: string; width: number }[] = [
  { name: 'Rolnictwo', resources: [0, 1, 2, 3, 4], border: '#7a5f00', width: 1 },
  { name: 'Hodowla', resources: [5, 6, 7], border: '#4a2410', width: 1 },
  { name: 'Las', resources: [8, 9], border: '#0c3a1f', width: 1 },
  { name: 'Energia', resources: [10, 11, 12, 13], border: '#ffcc00', width: 2 },
  { name: 'Metale', resources: [14, 15, 16, 17, 18, 19, 20, 21, 22, 23], border: '#00d8ff', width: 2 },
  { name: 'Przemysł', resources: [24, 25, 26], border: '#1d2f4a', width: 1 },
];

/** Ląd prowincji, zanim faza 3 (`polish`) przydzieli zasoby. */
export const RESOURCE_PENDING = [118, 124, 116] as const;
/** Prowincja z zasobem odznaczonym w legendzie i granica między takimi prowincjami. */
export const RESOURCE_OFF = [84, 90, 96] as const;
export const RESOURCE_OFF_BORDER = [66, 71, 77] as const;

const rgb = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
const RESOURCE_RGB = RESOURCES.map((r) => rgb(r.color));
const GROUP_OF = RESOURCES.map((_, r) => RESOURCE_GROUPS.findIndex((g) => g.resources.includes(r)));
const GROUP_BORDER_RGB = RESOURCE_GROUPS.map((g) => rgb(g.border));

/** Wygląd prowincji na mapie zasobów (indeks `id - 1`): wypełnienie i obwódka (RGB) oraz grubość obwódki (0 = bez). */
export interface ResourceStyle {
  fill: Float32Array;
  border: Float32Array;
  width: Uint8Array;
}

/** Wygląd prowincji na mapie zasobów; `enabled` – zasoby zaznaczone w legendzie. Null przed fazą 3. */
export function resourceStyle(map: MapPayload, enabled: readonly boolean[]): ResourceStyle | null {
  if (!map.polished) return null;
  const n = map.provinces.length;
  const style = { fill: new Float32Array(n * 3), border: new Float32Array(n * 3), width: new Uint8Array(n) };
  map.provinces.forEach((p, k) => {
    const r = p.resource;
    if (enabled[r] === false || !RESOURCE_RGB[r]) {
      style.fill.set(RESOURCE_OFF, k * 3);
      return;
    }
    const g = GROUP_OF[r];
    style.fill.set(RESOURCE_RGB[r], k * 3);
    style.border.set(GROUP_BORDER_RGB[g], k * 3);
    style.width[k] = RESOURCE_GROUPS[g].width;
  });
  return style;
}
