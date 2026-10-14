import type { MapPayload } from '../worker/protocol';

type Rgb = readonly [number, number, number];

/** Kolor granicy prowincji na mapie terenu – jak `BORDER` w CLI (crates/mapgen/src/bin/mapgen.rs). */
export const BORDER: Rgb = [44, 44, 48];
/**
 * Mapa polityczna – jak `POLITICAL_*` w CLI. Prowincja niczyja jest szara (prowincje graczy maluje
 * nakładka właścicieli, `render/owners.ts`), granica prowincji – ciemniejsza szarość.
 */
export const POLITICAL_UNOWNED: Rgb = [200, 198, 192];
export const POLITICAL_BORDER: Rgb = [150, 147, 140];
export const POLITICAL_SEA: Rgb = [128, 166, 200];
export const POLITICAL_LAKE: Rgb = [118, 158, 196];
/** Góry (niczyje, nieprzechodnie) na mapie politycznej – jak `POLITICAL_MOUNTAIN` w CLI. */
export const POLITICAL_MOUNTAIN: Rgb = [148, 140, 130];
/** Lodowiec (niczyj, nieprzechodni) na mapie politycznej – jak `POLITICAL_ICE` w CLI. */
export const POLITICAL_ICE: Rgb = [226, 232, 238];

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
