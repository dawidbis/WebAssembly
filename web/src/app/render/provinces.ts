import type { MapPayload } from '../worker/protocol';

type Rgb = readonly [number, number, number];

/** Kolor granicy prowincji na mapie terenu – jak `BORDER` w CLI (crates/mapgen/src/bin/mapgen.rs). */
export const BORDER: Rgb = [44, 44, 48];
/** Granica, morze i jeziora na mapie politycznej – jak `POLITICAL_*` w CLI. */
export const POLITICAL_BORDER: Rgb = [150, 24, 24];
export const POLITICAL_SEA: Rgb = [128, 166, 200];
export const POLITICAL_LAKE: Rgb = [118, 158, 196];
/** Góry (niczyje, nieprzechodnie) na mapie politycznej – jak `POLITICAL_MOUNTAIN` w CLI. */
export const POLITICAL_MOUNTAIN: Rgb = [148, 140, 130];
/** Lodowiec (niczyj, nieprzechodni) na mapie politycznej – jak `POLITICAL_ICE` w CLI. */
export const POLITICAL_ICE: Rgb = [226, 232, 238];
/** Kolory prowincji na mapie politycznej; sąsiednie prowincje zawsze w różnych kolorach (jak `POLITICAL` w CLI). */
const POLITICAL: readonly Rgb[] = [
  [226, 200, 150],
  [186, 212, 156],
  [216, 172, 172],
  [200, 186, 228],
  [228, 218, 150],
  [234, 184, 204],
  [238, 188, 140],
  [172, 212, 200],
];

/** Ten sam hash co `tile_hash` w CLI. */
function tileHash(x: number, y: number): number {
  let h = Math.imul(x, 0x9e3779b1) ^ Math.imul(y, 0x85ebca77);
  h = Math.imul(h ^ (h >>> 15), 0x2c1b3c6d);
  return ((h ^ (h >>> 12)) >>> 0) / 4294967295;
}

/**
 * Granica prowincji (jak `province_border` w CLI): kafel, którego prawy albo dolny sąsiad należy
 * do innej prowincji. Linia ma grubość jednego kafla i leży po stronie lewej/górnej prowincji.
 * Brzeg morza i jezior nie jest granicą.
 */
export function provinceBorder(province: Uint16Array, w: number, h: number, i: number): boolean {
  const p = province[i];
  if (p === 0) return false;
  const x = i % w;
  if (x + 1 < w) {
    const q = province[i + 1];
    if (q !== 0 && q !== p) return true;
  }
  if (i + w < w * h) {
    const q = province[i + w];
    if (q !== 0 && q !== p) return true;
  }
  return false;
}

const colorCache = new WeakMap<MapPayload, Float32Array>();

/**
 * Kolory prowincji na mapie politycznej (RGB na prowincję, indeks `id - 1`), jak `political_colors`
 * w CLI: zachłanne kolorowanie grafu sąsiedztwa – prowincja dostaje pierwszy kolor (od przesuniętego
 * o hash numeru) niezajęty przez sąsiadów, do tego lekka różnica jasności.
 */
export function politicalColors(map: MapPayload): Float32Array {
  const cached = colorCache.get(map);
  if (cached) return cached;
  const { width: w, height: h, province } = map;
  const count = map.provinces.length;
  const adj: number[][] = Array.from({ length: count }, () => []);
  const link = (p: number, q: number) => {
    if (q !== 0 && q !== p && !adj[p - 1].includes(q)) {
      adj[p - 1].push(q);
      adj[q - 1].push(p);
    }
  };
  for (let i = 0; i < w * h; i++) {
    const p = province[i];
    if (p === 0) continue;
    if ((i % w) + 1 < w) link(p, province[i + 1]);
    if (i + w < w * h) link(p, province[i + w]);
  }
  const k = POLITICAL.length;
  const color = new Uint8Array(count).fill(255);
  const out = new Float32Array(count * 3);
  for (let p = 0; p < count; p++) {
    const start = Math.floor(tileHash(p + 1, 3) * k);
    let chosen = start % k;
    for (let o = 0; o < k; o++) {
      const c = (start + o) % k;
      if (!adj[p].some((q) => color[q - 1] === c)) {
        chosen = c;
        break;
      }
    }
    color[p] = chosen;
    const light = 0.94 + 0.1 * tileHash(p + 1, 7);
    const base = POLITICAL[chosen];
    for (let c = 0; c < 3; c++) out[p * 3 + c] = Math.min(255, base[c] * light);
  }
  colorCache.set(map, out);
  return out;
}

/**
 * Nakładka z granicami prowincji: szare kafle granic, reszta przezroczysta. Kafle są
 * nieprzezroczyste – krycie granicy ustawia renderer (alpha warstwy, suwak „Krycie granic”).
 */
export function paintProvinceBorders(map: MapPayload): Uint8ClampedArray<ArrayBuffer> {
  const { width: w, height: h, province } = map;
  const out = new Uint8ClampedArray(w * h * 4);
  for (let i = 0; i < w * h; i++) {
    if (!provinceBorder(province, w, h, i)) continue;
    const o = i * 4;
    out[o] = BORDER[0];
    out[o + 1] = BORDER[1];
    out[o + 2] = BORDER[2];
    out[o + 3] = 255;
  }
  return out;
}
