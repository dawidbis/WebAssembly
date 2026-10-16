import type { MapPayload } from '../worker/protocol';

/** Ikony energii na mapie zasobów i w legendzie. */
export type ResourceIcon = 'bolt' | 'drop' | 'radiation';

/** Rodzaj zasobu – opis palety: cenniejsze rodzaje mają żywsze kolory (filtr w legendzie – `SUPPLY_CATEGORIES`). */
export type ResourceKind = 'basic' | 'strategic' | 'luxury';

/** Zasób prowincji na mapie zasobów i w ramce prowincji. */
export interface ResourceInfo {
  name: string;
  /** Kolor wypełnienia prowincji na mapie zasobów (hex). */
  color: string;
  kind: ResourceKind;
  /** Ikona na środku prowincji (energia). */
  icon?: ResourceIcon;
  /** Symbol pierwiastka albo wzór (metale, chemia) na środku prowincji i na ikonach. */
  symbol?: string;
  /** Piktogram (ścieżka SVG 24 × 24, rysowana białą linią z czarnym obrysem) – rolnictwo, hodowla, las. */
  glyph?: string;
}

/**
 * Zasoby (realia dwudziestolecia) w kolejności numerów `Province.resource` – jak `RESOURCE_NAMES` w
 * `crates/mapgen/src/resource_table.rs` (generuje go `tools/resources/chances.mjs`; zmieniaj oba).
 * Podstawowe rolnictwo i hodowla są przygaszone; energia ma ikony, metale symbole pierwiastków.
 * Każdy kolor odbiega od gór, morza, lodu (`POLITICAL_*`), szarości wyłączonych i od pozostałych
 * o ΔE ≥ 12 (poza bawełną i lodem – bawełny nie ma w klimacie polarnym).
 */
export const RESOURCES: readonly ResourceInfo[] = [
  { name: 'Zboża', color: '#d5c384', kind: 'basic', glyph: 'M12 21V7M12 7c-2.2-.6-3.2-2.4-3-4.5 2.2.2 3.2 2 3 4.5zm0 0c2.2-.6 3.2-2.4 3-4.5-2.2.2-3.2 2-3 4.5zM12 12c-2.2-.6-3.2-2.4-3-4.5 2.2.2 3.2 2 3 4.5zm0 0c2.2-.6 3.2-2.4 3-4.5-2.2.2-3.2 2-3 4.5zM12 17c-2.2-.6-3.2-2.4-3-4.5 2.2.2 3.2 2 3 4.5zm0 0c2.2-.6 3.2-2.4 3-4.5-2.2.2-3.2 2-3 4.5z' },
  { name: 'Ryż', color: '#dfe5ae', kind: 'basic', glyph: 'M4 12h16a8 8 0 0 1-16 0zM8 9c0-2 1-3.5 2.5-4.5M12 9c0-2 1-3.5 2.5-4.5M16 9c0-2 1-3.5 2.5-4.5' },
  { name: 'Ziemniaki', color: '#a89c6c', kind: 'basic', glyph: 'M6.5 9c2-4.5 9.5-5.5 11.5-1s-.5 10-5.5 10.5S4.5 13.5 6.5 9zM10 10.5h.01M14 9.5h.01M13 14h.01' },
  { name: 'Bawełna', color: '#f6f3ea', kind: 'basic', glyph: 'M12 5a3 3 0 0 1 3 3 3 3 0 0 1 0 6 3 3 0 0 1-6 0 3 3 0 0 1 0-6 3 3 0 0 1 3-3zM12 14v7M9 18l3-2 3 2' },
  { name: 'Włókna roślinne', color: '#a3b38c', kind: 'basic', glyph: 'M6 21c2.5-6 2.5-12 0-18M12 21c2.5-6 2.5-12 0-18M18 21c2.5-6 2.5-12 0-18' },
  { name: 'Tytoń', color: '#b8762e', kind: 'luxury', glyph: 'M5 19C5 10 11 4 19 4c0 8-6 14-14 15zM5 19l9-9M9 15h4M12 12V8' },
  { name: 'Trzcina cukrowa', color: '#d9506e', kind: 'luxury', glyph: 'M9 21V3M15 21V3M9 9h0M6 6l3 3M18 9l-3 3M6 15l3-1M18 5l-3 2M7 21h4M13 21h4' },
  { name: 'Kawa', color: '#6b3a1e', kind: 'luxury', glyph: 'M12 3c4 0 7 4 7 9s-3 9-7 9-7-4-7-9 3-9 7-9zM12 3c-2.5 4 2.5 10 0 18' },
  { name: 'Herbata', color: '#8fbf3a', kind: 'luxury', glyph: 'M5 9h11v5a5 5 0 0 1-10 0zM16 10h2a2 2 0 0 1 0 4h-2M8 6c0-1.2 1-2 2-3M12 6c0-1.2 1-2 2-3' },
  { name: 'Kauczuk', color: '#5d8a2c', kind: 'strategic', glyph: 'M8 21V9M8 9c-3 0-4-3-3-5 2 0 3 2 3 5zm0 3c3 0 5-2 5-5-2 0-5 2-5 5zM17 11c-2 3-3 4.5-3 6a3 3 0 0 0 6 0c0-1.5-1-3-3-6z' },
  { name: 'Bydło', color: '#7f5947', kind: 'basic', glyph: 'M6 7 3 5M18 7l3-2M6 7h12v5a6 6 0 0 1-12 0zM9.5 16.5h5M10 10h.01M14 10h.01' },
  { name: 'Konie', color: '#c2643a', kind: 'strategic', glyph: 'M7 20v-8a5 5 0 0 1 10 0v8M5 20h4M15 20h4M7 13h.01M7 16h.01M17 13h.01M17 16h.01' },
  { name: 'Wełna', color: '#d2b9a8', kind: 'basic', glyph: 'M12 4a8 8 0 1 0 0 16 8 8 0 1 0 0-16zM5 9c4 1 9.5 5.5 12 10M8 5c3 3 8 7.5 11 9M4.5 14c4-1 8 1 10.5 5.5' },
  { name: 'Futra', color: '#83717b', kind: 'luxury', glyph: 'M8 15c0-2.5 1.8-3.5 4-3.5s4 1 4 3.5-2 4.5-4 4.5-4-2-4-4.5zM5.5 10h.01M9.5 6.5h.01M14.5 6.5h.01M18.5 10h.01' },
  { name: 'Drewno', color: '#2d6b43', kind: 'basic', glyph: 'M12 3 6 12h3l-3.5 5h13L15 12h3zM12 17v4' },
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
  { name: 'Surowce nawozowe', color: '#86d4f2', kind: 'basic', glyph: 'M6 21c-.6-4 .4-8 2.5-10h7c2.1 2 3.1 6 2.5 10zM8.5 11 7 7h10l-1.5 4M12 19v-4.5M12 16c-1.8 0-3-1.1-3-2.8 1.8 0 3 1.1 3 2.8zm0-1.5c1.8 0 3-1.1 3-2.8-1.8 0-3 1.1-3 2.8z' },
  { name: 'Cement', color: '#ddd0ad', kind: 'basic', glyph: 'M3 20h18V10H3zM3 15h18M8 10v5M16 10v5M12 15v5M5 10V5h14v5M12 5v5' },
  { name: 'Saletra', color: '#b52fc0', kind: 'strategic', glyph: 'M12 3l3.5 5L12 21 8.5 8zM8.5 8h7M6 10l2 3.5L6.5 20 4 13.5zM18 10l-2 3.5 1.5 6.5 2.5-6.5z' },
  { name: 'Siarka', color: '#eae35a', kind: 'strategic', glyph: 'M3 20l5-9h8l5 9zM8 11l2 2 2-2 2 2 2-2M10 8c0-1.5 1.5-2 1-4M14 8c0-1.5 1.5-2 1-4' },
  // Dopisane na końcu (numery starszych bez zmian) – jedzenie na prowiant.
  { name: 'Kukurydza', color: '#e0a526', kind: 'basic', glyph: 'M12 3c2.8 0 4.5 3.5 4.5 8.5S14.8 20 12 20s-4.5-3.5-4.5-8.5S9.2 3 12 3zM9 8h6M8.5 12h7M9 16h6M12 3v17M7.5 13C5 13 4 16 4 21c3-1 6-3 8-6M16.5 13c2.5 0 3.5 3 3.5 8-3-1-6-3-8-6' },
  { name: 'Drób', color: '#e89a7a', kind: 'basic', glyph: 'M9 10a3 3 0 1 1 6 0c3 1 5 4 4 8H6c-1-4 0-6.5 3-8zM15 7l2.5-1-1 2.5M12 5.5V4M9.5 21v-3M14.5 21v-3M10.5 9h.01' },
  { name: 'Trzoda chlewna', color: '#e07a9a', kind: 'basic', glyph: 'M4 13.5a7 5.5 0 1 0 14 0 7 5.5 0 1 0-14 0zM18 12h2.5v3H18M7.5 8.5 6.5 5l3.5 3M10 12h.01M7.5 18.5V21M14.5 18.5V21' },
  { name: 'Ryby', color: '#3f8fd8', kind: 'basic', glyph: 'M2.5 12c3-4.5 8.5-5.5 13-3l5.5-3.5v13L15.5 15c-4.5 2.5-10 1.5-13-3zM7.5 11h.01M12 8.5c1 2 1 5 0 7' },
];

/**
 * Kategorie surowców według zaopatrzenia, do którego są potrzebne (przyciski filtra w legendzie) –
 * jak receptury w docs/gdd/zaopatrzenie.md (z zamiennikami). `synthetic` – surowce, które dochodzą
 * po przekroczeniu progu sumy poziomów uniwersytetów (np. paliwo syntetyczne z węgla). „Pozostałe” –
 * surowce spoza receptur (złoto, uran, nawozy). Przy etapie fabryk receptury przejdą do rdzenia.
 */
export const SUPPLY_CATEGORIES: readonly {
  name: string;
  resources: readonly number[];
  synthetic?: readonly { universities: number; resources: readonly number[] }[];
}[] = [
  { name: 'Lekkie wojskowe', resources: [19, 25, 3, 12, 4, 33, 34, 26, 27] },
  { name: 'Standardowe wojskowe', resources: [19, 20, 21, 22, 33, 34, 26, 27, 15, 16] },
  { name: 'Ciężkie wojskowe', resources: [19, 21, 22, 23, 24, 15, 9, 28], synthetic: [{ universities: 15, resources: [17] }] },
  { name: 'Paliwo', resources: [17], synthetic: [{ universities: 10, resources: [15, 16] }] },
  { name: 'Prowiant', resources: [0, 1, 35, 2, 10, 37, 36, 38] },
  { name: 'Podstawowe cywilne', resources: [3, 12, 4, 14, 15, 16, 17, 11] },
  { name: 'Luksusowe cywilne', resources: [5, 6, 7, 8, 13, 12, 25, 29] },
  { name: 'Materiały inżynieryjne', resources: [32, 14, 19, 17, 11] },
  { name: 'Pozostałe', resources: [30, 18, 31] },
];

/** Surowce kategorii przy danej sumie poziomów uniwersytetów (syntetyki dochodzą po progach). */
export function categoryResources(category: (typeof SUPPLY_CATEGORIES)[number], universities: number): number[] {
  const extra = (category.synthetic ?? []).filter((s) => universities >= s.universities).flatMap((s) => s.resources);
  return [...new Set([...category.resources, ...extra])];
}

/** Grupy zasobów w legendzie; `marks` – znak na mapie na środku prowincji (ikona albo symbol pierwiastka). */
export const RESOURCE_GROUPS: readonly { name: string; resources: readonly number[]; marks?: boolean }[] = [
  { name: 'Rolnictwo', resources: [0, 1, 2, 35, 3, 4, 5, 6, 7, 8, 9] },
  { name: 'Hodowla i połowy', resources: [10, 36, 37, 11, 12, 13, 38] },
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
