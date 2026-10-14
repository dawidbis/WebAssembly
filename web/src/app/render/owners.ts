import type { MapPayload } from '../worker/protocol';
import { provinceBorder } from './provinces';

type Rgb = readonly [number, number, number];

/**
 * Kolory państw (numer państwa `n` → `NATION_COLORS[(n - 1) % długość]`). Nasycone, żeby odcinały się
 * od szarych prowincji niczyich (`POLITICAL_UNOWNED` w `render/provinces.ts`).
 */
export const NATION_COLORS: readonly Rgb[] = [
  [206, 62, 54], // czerwony
  [52, 108, 196], // niebieski
  [58, 152, 76], // zielony
  [232, 168, 36], // bursztynowy
  [138, 78, 178], // fioletowy
  [36, 164, 168], // morski
  [226, 108, 168], // różowy
  [128, 88, 46], // brązowy
  [244, 124, 36], // pomarańczowy
  [84, 86, 160], // indygo
  [146, 186, 46], // limonkowy
  [176, 38, 104], // amarantowy
];

/** Jasność koloru państwa na granicy prowincji wewnątrz państwa i na granicy państwa. */
const PROVINCE_EDGE = 0.8;
const NATION_EDGE = 0.5;

export function nationColor(nation: number): Rgb {
  return NATION_COLORS[(nation - 1) % NATION_COLORS.length];
}

/** Kolor CSS państwa (ramka prowincji, panel). */
export function nationCss(nation: number): string {
  const [r, g, b] = nationColor(nation);
  return `rgb(${r}, ${g}, ${b})`;
}

/**
 * Nakładka właścicieli na mapę polityczną: kafel państwa w jego kolorze, kafel niczyj przezroczysty
 * (spod spodu widać szarą prowincję). Granica państwa – kafel, którego sąsiad (bok) z prowincji ma
 * innego właściciela – jest ciemna, granica prowincji wewnątrz państwa lekko przyciemniona.
 */
export function paintOwners(map: MapPayload, tiles: Uint8Array): Uint8ClampedArray<ArrayBuffer> {
  const { width: w, height: h, province } = map;
  const out = new Uint8ClampedArray(w * h * 4);
  for (let i = 0; i < w * h; i++) {
    const n = tiles[i];
    if (n === 0) continue;
    let k = 1;
    if (nationEdge(tiles, province, w, h, i)) k = NATION_EDGE;
    else if (provinceBorder(province, w, h, i)) k = PROVINCE_EDGE;
    const c = nationColor(n);
    const o = i * 4;
    out[o] = c[0] * k;
    out[o + 1] = c[1] * k;
    out[o + 2] = c[2] * k;
    out[o + 3] = 255;
  }
  return out;
}

/** Czy sąsiad kafla (bok) należy do prowincji i ma innego właściciela (także niczyj). */
function nationEdge(tiles: Uint8Array, province: Uint16Array, w: number, h: number, i: number): boolean {
  const n = tiles[i];
  const x = i % w;
  const other = (j: number) => province[j] !== 0 && tiles[j] !== n;
  return (x > 0 && other(i - 1)) || (x + 1 < w && other(i + 1)) || (i >= w && other(i - w)) || (i + w < w * h && other(i + w));
}
