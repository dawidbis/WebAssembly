import type { MapPayload } from '../worker/protocol';
import { flagIndex, flagOf, flagPalette } from './flags';

type Rgba = Uint8ClampedArray<ArrayBuffer>;

/** Domyślna grubość granicy państwa na mapie terenu (kafle, w głąb państwa) – suwak w panelu debugu. */
export const TERRITORY_BORDER_WIDTH = 3;

/**
 * Położenie nazwy państwa (jak w EU4): napis wzdłuż głównej osi największego kawałka terytorium,
 * lekko wygięty po krzywej `v = a·u² + b·u + c` (u – wzdłuż osi, v – w poprzek, w kaflach, od środka
 * `cx, cy`), od `u0` do `u1`. `thickness` – typowa grubość terytorium w poprzek osi (kafle).
 */
export interface TerritoryLabel {
  nation: number;
  cx: number;
  cy: number;
  angle: number;
  u0: number;
  u1: number;
  a: number;
  b: number;
  c: number;
  thickness: number;
}

/**
 * Warstwy terytoriów malowane w workerze: flagi (RGBA całej mapy, tylko na prośbę – gdy są widoczne)
 * i położenie nazw. Kolory, tło i granice państw liczy na żywo `TerritoryLayer` (GPU), więc nie ma ich tu.
 */
export interface TerritoryLayers {
  /** Mapa terenu: flaga państwa rozciągnięta na każdy spójny kawałek terytorium (null – nie proszono). */
  flags: Rgba | null;
  labels: TerritoryLabel[];
}

/** Spójne kawałki terytoriów (sąsiedztwo 4): kafle kolejnych kawałków w `order`, kawałek `k` od `start[k]`. */
interface Pieces {
  order: Int32Array;
  start: number[];
  nation: number[];
}

function pieces(tiles: Uint8Array, w: number, h: number): Pieces {
  const seen = new Uint8Array(w * h);
  const order = new Int32Array(w * h);
  const start: number[] = [];
  const nation: number[] = [];
  let end = 0;
  for (let i = 0; i < w * h; i++) {
    if (tiles[i] === 0 || seen[i]) continue;
    start.push(end);
    nation.push(tiles[i]);
    end = flood(tiles, w, h, i, seen, order, end);
  }
  start.push(end);
  return { order, start, nation };
}

/** Kawałek państwa z kafla `from` (BFS, sąsiedztwo 4) dopisany do `order` od `end`; wynik – nowy koniec. */
function flood(
  tiles: Uint8Array,
  w: number,
  h: number,
  from: number,
  seen: Uint8Array,
  order: Int32Array,
  end: number,
): number {
  const n = tiles[from];
  const size = w * h;
  seen[from] = 1;
  order[end++] = from;
  // Bez tablicy sąsiadów na kafel – przy milionach kafli to miliony małych obiektów do sprzątania.
  const visit = (k: number) => {
    if (!seen[k] && tiles[k] === n) {
      seen[k] = 1;
      order[end++] = k;
    }
  };
  for (let q = end - 1; q < end; q++) {
    const j = order[q];
    const x = j % w;
    if (x > 0) visit(j - 1);
    if (x + 1 < w) visit(j + 1);
    if (j >= w) visit(j - w);
    if (j + w < size) visit(j + w);
  }
  return end;
}

function put(out: Rgba, i: number, c: readonly [number, number, number]): void {
  const o = i * 4;
  out[o] = c[0];
  out[o + 1] = c[1];
  out[o + 2] = c[2];
  out[o + 3] = 255;
}

/** Flagi (gdy `withFlags`) i położenie nazw z właścicieli kafli (`tiles` – państwo na kafel, 0 = niczyje). */
export function paintTerritories(
  map: MapPayload,
  tiles: Uint8Array,
  withFlags: boolean,
): TerritoryLayers {
  const { width: w, height: h } = map;
  const parts = pieces(tiles, w, h);
  let flags: Rgba | null = null;
  if (withFlags) {
    flags = new Uint8ClampedArray(w * h * 4);
    paintFlags(flags, parts, w);
  }
  return { flags, labels: labels(parts, w) };
}

/** Flaga na każdy kawałek terytorium: prostokąt flagi 3:2 przykrywa prostokąt kawałka, środek na środku. */
function paintFlags(out: Rgba, parts: Pieces, w: number): void {
  for (let p = 0; p < parts.nation.length; p++) {
    let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
    for (let q = parts.start[p]; q < parts.start[p + 1]; q++) {
      const i = parts.order[q];
      const x = i % w;
      const y = (i - x) / w;
      x0 = Math.min(x0, x);
      x1 = Math.max(x1, x);
      y0 = Math.min(y0, y);
      y1 = Math.max(y1, y);
    }
    const bw = x1 - x0 + 1;
    const bh = y1 - y0 + 1;
    const s = Math.max(bw / 3, bh / 2); // kafle na jednostkę flagi
    const ox = x0 - (3 * s - bw) / 2;
    const oy = y0 - (2 * s - bh) / 2;
    const flag = flagOf(parts.nation[p]);
    const palette = flagPalette(flag);
    for (let q = parts.start[p]; q < parts.start[p + 1]; q++) {
      const i = parts.order[q];
      const x = i % w;
      const y = (i - x) / w;
      put(out, i, palette[flagIndex(flag, (x + 0.5 - ox) / (3 * s), (y + 0.5 - oy) / (2 * s))]);
    }
  }
}

/** Liczba przedziałów wzdłuż osi przy dopasowaniu krzywej napisu. */
const LABEL_BINS = 24;

/**
 * Nazwy państw: dla każdego państwa jego największy kawałek. Oś napisu – główna oś kafli (PCA),
 * zakres – 5.–95. percentyl rzutów na oś, krzywa – parabola przez środki przedziałów w poprzek osi.
 */
function labels(parts: Pieces, w: number): TerritoryLabel[] {
  const best = new Map<number, number>();
  for (let p = 0; p < parts.nation.length; p++) {
    const n = parts.nation[p];
    const size = parts.start[p + 1] - parts.start[p];
    const cur = best.get(n);
    if (cur === undefined || size > parts.start[cur + 1] - parts.start[cur]) best.set(n, p);
  }
  const out: TerritoryLabel[] = [];
  for (const [nation, p] of best) {
    const label = pieceLabel(parts, p, w, nation);
    if (label) out.push(label);
  }
  return out.sort((a, b) => a.nation - b.nation);
}

function pieceLabel(parts: Pieces, p: number, w: number, nation: number): TerritoryLabel | null {
  const from = parts.start[p];
  const to = parts.start[p + 1];
  const count = to - from;
  if (count < 4) return null;
  let sx = 0;
  let sy = 0;
  for (let q = from; q < to; q++) {
    const i = parts.order[q];
    sx += i % w;
    sy += Math.floor(i / w);
  }
  const cx = sx / count;
  const cy = sy / count;
  let cxx = 0;
  let cyy = 0;
  let cxy = 0;
  for (let q = from; q < to; q++) {
    const i = parts.order[q];
    const dx = (i % w) - cx;
    const dy = Math.floor(i / w) - cy;
    cxx += dx * dx;
    cyy += dy * dy;
    cxy += dx * dy;
  }
  // Główna oś w (−90°, 90°] – napis zawsze czytany od lewej.
  const angle = 0.5 * Math.atan2(2 * cxy, cxx - cyy);
  const cos = Math.cos(angle);
  const sin = Math.sin(angle);
  let umin = Infinity;
  let umax = -Infinity;
  for (let q = from; q < to; q++) {
    const i = parts.order[q];
    const u = ((i % w) - cx) * cos + (Math.floor(i / w) - cy) * sin;
    umin = Math.min(umin, u);
    umax = Math.max(umax, u);
  }
  const span = Math.max(1e-6, umax - umin);
  const bins = Array.from({ length: LABEL_BINS }, () => ({ n: 0, sv: 0, vmin: Infinity, vmax: -Infinity }));
  for (let q = from; q < to; q++) {
    const i = parts.order[q];
    const dx = (i % w) - cx;
    const dy = Math.floor(i / w) - cy;
    const u = dx * cos + dy * sin;
    const v = -dx * sin + dy * cos;
    const bin = bins[Math.min(LABEL_BINS - 1, Math.floor(((u - umin) / span) * LABEL_BINS))];
    bin.n++;
    bin.sv += v;
    bin.vmin = Math.min(bin.vmin, v);
    bin.vmax = Math.max(bin.vmax, v);
  }
  // 5. i 95. percentyl wzdłuż osi (z przedziałów).
  const binU = (k: number) => umin + ((k + 0.5) / LABEL_BINS) * span;
  let acc = 0;
  let k0 = 0;
  let k1 = LABEL_BINS - 1;
  for (let k = 0; k < LABEL_BINS; k++) {
    acc += bins[k].n;
    if (acc < count * 0.05) k0 = k + 1;
    if (acc <= count * 0.95) k1 = Math.min(LABEL_BINS - 1, k + 1);
  }
  const used = bins.slice(k0, k1 + 1).map((b, j) => ({ ...b, u: binU(k0 + j) })).filter((b) => b.n > 0);
  if (used.length === 0) return null;
  const [a, b, c] = fitParabola(used.map((x) => x.u), used.map((x) => x.sv / x.n), used.map((x) => x.n));
  const thick = used.map((x) => x.vmax - x.vmin + 1).sort((x, y) => x - y);
  const u0 = binU(k0) - (0.5 / LABEL_BINS) * span;
  const u1 = binU(k1) + (0.5 / LABEL_BINS) * span;
  // Wygięcie najwyżej na 12% długości napisu (strzałka łuku).
  const half = (u1 - u0) / 2;
  const sag = Math.abs(a) * half * half;
  const limit = 0.12 * (u1 - u0);
  const scale = sag > limit ? limit / sag : 1;
  return { nation, cx, cy, angle, u0, u1, a: a * scale, b: b * scale, c, thickness: thick[Math.floor(thick.length / 2)] };
}

/** Ważona regresja v = a·u² + b·u + c (przy mniej niż 3 punktach – prosta albo stała). */
function fitParabola(us: number[], vs: number[], ws: number[]): [number, number, number] {
  let [s0, s1, s2, s3, s4, t0, t1, t2] = [0, 0, 0, 0, 0, 0, 0, 0];
  for (let i = 0; i < us.length; i++) {
    const u = us[i];
    const v = vs[i];
    const wt = ws[i];
    s0 += wt;
    s1 += wt * u;
    s2 += wt * u * u;
    s3 += wt * u ** 3;
    s4 += wt * u ** 4;
    t0 += wt * v;
    t1 += wt * u * v;
    t2 += wt * u * u * v;
  }
  if (us.length >= 3) {
    // Układ normalny 3 × 3 (reguła Cramera).
    const det3 = (m: number[]) =>
      m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6]) + m[2] * (m[3] * m[7] - m[4] * m[6]);
    const m = [s4, s3, s2, s3, s2, s1, s2, s1, s0];
    const d = det3(m);
    if (Math.abs(d) > 1e-9) {
      const a = det3([t2, s3, s2, t1, s2, s1, t0, s1, s0]) / d;
      const b = det3([s4, t2, s2, s3, t1, s1, s2, t0, s0]) / d;
      const c = det3([s4, s3, t2, s3, s2, t1, s2, s1, t0]) / d;
      return [a, b, c];
    }
  }
  const d = s0 * s2 - s1 * s1;
  if (us.length >= 2 && Math.abs(d) > 1e-9) return [0, (s0 * t1 - s1 * t0) / d, (s2 * t0 - s1 * t1) / d];
  return [0, 0, s0 > 0 ? t0 / s0 : 0];
}
