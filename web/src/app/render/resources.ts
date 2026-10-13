import type { MapPayload } from '../worker/protocol';

/** Ikony energii na mapie zasobów i w legendzie. */
export type ResourceIcon = 'bolt' | 'drop' | 'radiation';

/** Rodzaj zasobu – filtr w legendzie (`RESOURCE_KINDS`); cenniejsze rodzaje mają żywsze kolory. */
export type ResourceKind = 'basic' | 'strategic' | 'luxury';

/** Zasób prowincji na mapie zasobów i w ramce prowincji. */
export interface ResourceInfo {
  name: string;
  /** Kolor wypełnienia prowincji na mapie zasobów (hex). */
  color: string;
  kind: ResourceKind;
  /** Ikona na środku prowincji (energia). */
  icon?: ResourceIcon;
  /** Symbol pierwiastka na środku prowincji (metale). */
  symbol?: string;
}

/**
 * Zasoby (realia dwudziestolecia) w kolejności numerów `Province.resource` – jak `RESOURCE_NAMES` w
 * `crates/mapgen/src/resource_table.rs` (generuje go `tools/resources/chances.mjs`; zmieniaj oba).
 * Podstawowe rolnictwo i hodowla są przygaszone; energia ma ikony, metale symbole pierwiastków.
 * Każdy kolor odbiega od gór, morza, lodu (`POLITICAL_*`), szarości wyłączonych i od pozostałych
 * o ΔE ≥ 12 (poza bawełną i lodem – bawełny nie ma w klimacie polarnym).
 */
export const RESOURCES: readonly ResourceInfo[] = [
  { name: 'Zboża', color: '#d5c384', kind: 'basic' },
  { name: 'Ryż', color: '#dfe5ae', kind: 'basic' },
  { name: 'Ziemniaki', color: '#a89c6c', kind: 'basic' },
  { name: 'Bawełna', color: '#f6f3ea', kind: 'basic' },
  { name: 'Włókna roślinne', color: '#a3b38c', kind: 'basic' },
  { name: 'Tytoń', color: '#b8762e', kind: 'luxury' },
  { name: 'Trzcina cukrowa', color: '#d9506e', kind: 'luxury' },
  { name: 'Kawa', color: '#6b3a1e', kind: 'luxury' },
  { name: 'Herbata', color: '#8fbf3a', kind: 'luxury' },
  { name: 'Kauczuk', color: '#5d8a2c', kind: 'strategic' },
  { name: 'Bydło', color: '#7f5947', kind: 'basic' },
  { name: 'Konie', color: '#c2643a', kind: 'strategic' },
  { name: 'Wełna', color: '#d2b9a8', kind: 'basic' },
  { name: 'Futra', color: '#83717b', kind: 'luxury' },
  { name: 'Drewno', color: '#2d6b43', kind: 'basic' },
  { name: 'Węgiel kamienny', color: '#3d3d42', kind: 'strategic', icon: 'bolt' },
  { name: 'Węgiel brunatny', color: '#4b3424', kind: 'strategic', icon: 'bolt' },
  { name: 'Ropa naftowa', color: '#000000', kind: 'strategic', icon: 'drop' },
  { name: 'Uran', color: '#7dff3a', kind: 'strategic', icon: 'radiation' },
  { name: 'Rudy żelaza', color: '#b2372a', kind: 'strategic', symbol: 'Fe' },
  { name: 'Mangan', color: '#ee93c6', kind: 'strategic', symbol: 'Mn' },
  { name: 'Chrom', color: '#bdb6d6', kind: 'strategic', symbol: 'Cr' },
  { name: 'Nikiel', color: '#7a9a8c', kind: 'strategic', symbol: 'Ni' },
  { name: 'Wolfram', color: '#5576b0', kind: 'strategic', symbol: 'W' },
  { name: 'Molibden', color: '#25408c', kind: 'strategic', symbol: 'Mo' },
  { name: 'Miedź', color: '#d4772f', kind: 'strategic', symbol: 'Cu' },
  { name: 'Ołów', color: '#6b7a9c', kind: 'basic', symbol: 'Pb' },
  { name: 'Cynk', color: '#3fa4a8', kind: 'basic', symbol: 'Zn' },
  { name: 'Aluminium', color: '#b8c2cc', kind: 'strategic', symbol: 'Al' },
  { name: 'Cyna', color: '#9d8fae', kind: 'strategic', symbol: 'Sn' },
  { name: 'Złoto', color: '#ffd23f', kind: 'luxury', symbol: 'Au' },
  { name: 'Surowce nawozowe', color: '#86d4f2', kind: 'basic' },
  { name: 'Cement', color: '#ddd0ad', kind: 'basic' },
  { name: 'Saletra', color: '#b52fc0', kind: 'strategic' },
  { name: 'Siarka', color: '#eae35a', kind: 'strategic' },
];

/** Rodzaje zasobów (przyciski filtra w legendzie). */
export const RESOURCE_KINDS: Record<ResourceKind, string> = {
  basic: 'Podstawowe',
  strategic: 'Strategiczne',
  luxury: 'Luksusowe',
};

/** Grupy zasobów w legendzie; `marks` – znak na mapie na środku prowincji (ikona albo symbol pierwiastka). */
export const RESOURCE_GROUPS: readonly { name: string; resources: readonly number[]; marks?: boolean }[] = [
  { name: 'Rolnictwo', resources: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9] },
  { name: 'Hodowla', resources: [10, 11, 12, 13] },
  { name: 'Las', resources: [14] },
  { name: 'Energia', resources: [15, 16, 17, 18], marks: true },
  { name: 'Metale', resources: [19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30], marks: true },
  { name: 'Chemia', resources: [31, 32, 33, 34] },
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
/** Cienka granica prowincji na mapie zasobów. */
export const RESOURCE_PLAIN_BORDER = [52, 57, 66] as const;

const rgb = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
const RESOURCE_RGB = RESOURCES.map((r) => rgb(r.color));

/**
 * Kolory prowincji na mapie zasobów (RGB, indeks `id - 1`); `enabled` – zasoby zaznaczone w legendzie
 * (odznaczone są szare). Null przed fazą 3.
 */
export function resourceColors(map: MapPayload, enabled: readonly boolean[]): Float32Array | null {
  if (!map.polished) return null;
  const out = new Float32Array(map.provinces.length * 3);
  map.provinces.forEach((p, k) => {
    const r = p.resource;
    out.set(enabled[r] === false || !RESOURCE_RGB[r] ? RESOURCE_OFF : RESOURCE_RGB[r], k * 3);
  });
  return out;
}
