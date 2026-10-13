import type { MapPayload } from '../worker/protocol';

/** Ikony energii na mapie zasobów i w legendzie. */
export type ResourceIcon = 'bolt' | 'drop' | 'radiation';

/** Zasób prowincji na mapie zasobów i w ramce prowincji. */
export interface ResourceInfo {
  name: string;
  /** Kolor wypełnienia prowincji na mapie zasobów (hex). */
  color: string;
  /** Ikona na środku prowincji (energia). */
  icon?: ResourceIcon;
  /** Symbol pierwiastka na środku prowincji (metale). */
  symbol?: string;
}

/**
 * Zasoby w kolejności numerów `Province.resource` – jak `RESOURCE_NAMES` w
 * `crates/mapgen/src/resource_table.rs` (generuje go `tools/resources/chances.mjs`; zmieniaj oba).
 * Rolnictwo (poza kauczukiem i uprawami plantacyjnymi) i hodowla są przygaszone – to najmniej istotne
 * kategorie. Energia ma ikony, metale symbole pierwiastków. Każdy kolor odbiega od gór, morza, lodu
 * (`POLITICAL_*`), szarości wyłączonych i od pozostałych o ΔE ≥ 13 (poza bawełną i lodem – bawełny
 * nie ma w klimacie polarnym).
 */
export const RESOURCES: readonly ResourceInfo[] = [
  { name: 'Zboża', color: '#d5c384' },
  { name: 'Ryż', color: '#dfe5ae' },
  { name: 'Bawełna', color: '#f6f3ea' },
  { name: 'Uprawy plantacyjne', color: '#d9506e' },
  { name: 'Kauczuk', color: '#5d8a2c' },
  { name: 'Bydło', color: '#7f5947' },
  { name: 'Skóry', color: '#bfa892' },
  { name: 'Futra', color: '#83717b' },
  { name: 'Drewno', color: '#2d6b43' },
  { name: 'Celuloza', color: '#8fd0a4' },
  { name: 'Węgiel kamienny', color: '#3d3d42', icon: 'bolt' },
  { name: 'Węgiel brunatny', color: '#4b3424', icon: 'bolt' },
  { name: 'Ropa naftowa', color: '#000000', icon: 'drop' },
  { name: 'Uran', color: '#7dff3a', icon: 'radiation' },
  { name: 'Rudy żelaza', color: '#b2372a', symbol: 'Fe' },
  { name: 'Mangan', color: '#ee93c6', symbol: 'Mn' },
  { name: 'Chrom', color: '#bdb6d6', symbol: 'Cr' },
  { name: 'Nikiel', color: '#7a9a8c', symbol: 'Ni' },
  { name: 'Wolfram', color: '#5576b0', symbol: 'W' },
  { name: 'Molibden', color: '#25408c', symbol: 'Mo' },
  { name: 'Miedź', color: '#d4772f', symbol: 'Cu' },
  { name: 'Ołów', color: '#6b7a9c', symbol: 'Pb' },
  { name: 'Cynk', color: '#3fa4a8', symbol: 'Zn' },
  { name: 'Aluminium', color: '#b8c2cc', symbol: 'Al' },
  { name: 'Nawozy mineralne', color: '#86d4f2' },
  { name: 'Cement', color: '#ddd0ad' },
  { name: 'Materiały wybuchowe', color: '#b52fc0' },
];

/**
 * Grupy zasobów (legenda) i obwódka prowincji na mapie zasobów: kolor i grubość w kaflach (0 = bez).
 * Rolnictwo, hodowla i las – jasnoszara z nutą koloru kategorii; energia i metale bez obwódki
 * (wyróżniają je ikony i symbole pierwiastków).
 */
export const RESOURCE_GROUPS: readonly { name: string; resources: readonly number[]; border: string; width: number }[] = [
  { name: 'Rolnictwo', resources: [0, 1, 2, 3, 4], border: '#d6d3bf', width: 1 },
  { name: 'Hodowla', resources: [5, 6, 7], border: '#d4c9bf', width: 1 },
  { name: 'Las', resources: [8, 9], border: '#c3d1c3', width: 1 },
  { name: 'Energia', resources: [10, 11, 12, 13], border: '', width: 0 },
  { name: 'Metale', resources: [14, 15, 16, 17, 18, 19, 20, 21, 22, 23], border: '', width: 0 },
  { name: 'Chemia', resources: [24, 25, 26], border: '#1d2f4a', width: 1 },
];

/** Kształt ikony w układzie 24 × 24 (ścieżka SVG – ta sama w legendzie i na mapie). */
export interface IconShape {
  d: string;
  fill: string;
  stroke?: string;
  /** Grubość obrysu w jednostkach układu 24 × 24. */
  strokeWidth?: number;
}

/** Wycinek pierścienia (łopatka znaku radioaktywności) wokół środka (12, 12); kąty w stopniach, y w dół. */
function ringSector(from: number, to: number, r1: number, r2: number): string {
  const pt = (r: number, a: number) => {
    const t = (a * Math.PI) / 180;
    return `${(12 + r * Math.cos(t)).toFixed(2)} ${(12 + r * Math.sin(t)).toFixed(2)}`;
  };
  return `M${pt(r1, from)}L${pt(r2, from)}A${r2} ${r2} 0 0 1 ${pt(r2, to)}L${pt(r1, to)}A${r1} ${r1} 0 0 0 ${pt(r1, from)}Z`;
}

const INK = '#141414';

/** Ikony energii: piorun (węgiel), kropla (ropa – biała obwódka, czarny środek), znak radioaktywności (uran). */
export const ICONS: Record<ResourceIcon, readonly IconShape[]> = {
  bolt: [{ d: 'M14 1.5 4 13.5h6.5L9 22.5l11-13h-6.8z', fill: '#ffd400', stroke: INK, strokeWidth: 1.4 }],
  drop: [{ d: 'M12 2.2C9.2 6.6 5.2 10.6 5.2 15.2a6.8 6.8 0 0 0 13.6 0c0-4.6-4-8.6-6.8-13z', fill: '#000000', stroke: '#ffffff', strokeWidth: 1.8 }],
  radiation: [
    { d: 'M12 1a11 11 0 1 0 0 22a11 11 0 1 0 0-22z', fill: '#ffd400', stroke: INK, strokeWidth: 1 },
    { d: 'M14.1 12a2.1 2.1 0 1 0-4.2 0a2.1 2.1 0 1 0 4.2 0z', fill: INK },
    { d: [-150, -30, 90].map((a) => ringSector(a - 30, a + 30, 3.4, 9.2)).join(''), fill: INK },
  ],
};

/** Ląd prowincji, zanim faza 3 (`polish`) przydzieli zasoby. */
export const RESOURCE_PENDING = [118, 124, 116] as const;
/** Prowincja z zasobem odznaczonym w legendzie. */
export const RESOURCE_OFF = [86, 94, 107] as const;
/** Cienka granica prowincji bez obwódki (energia, metale, odznaczone). */
export const RESOURCE_PLAIN_BORDER = [52, 57, 66] as const;

const rgb = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
const RESOURCE_RGB = RESOURCES.map((r) => rgb(r.color));
const GROUP_OF = RESOURCES.map((_, r) => RESOURCE_GROUPS.findIndex((g) => g.resources.includes(r)));
const GROUP_BORDER_RGB = RESOURCE_GROUPS.map((g) => (g.width > 0 ? rgb(g.border) : [0, 0, 0]));

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
