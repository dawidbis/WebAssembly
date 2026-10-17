import type { MapPayload } from '../worker/protocol';
import { flagOf, flagPixel } from './flags';
import { nationColor } from './nations';
import { provinceBorder } from './provinces';

type Rgb = readonly [number, number, number];
type Rgba = Uint8ClampedArray<ArrayBuffer>;

/** Mapa polityczna: jasność koloru państwa na granicy prowincji wewnątrz państwa i na granicy państwa. */
const PROVINCE_EDGE = 0.8;
const NATION_EDGE = 0.5;
/** Mapa terenu: krycie granicy państwa w najgłębszym jej kaflu (przy krawędzi – pełne). */
const BORDER_INNER = 0.35;
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

/** Warstwy terytoriów (RGBA całej mapy) i położenie nazw. */
export interface TerritoryLayers {
  /** Mapa polityczna: kolor państwa z ciemną granicą państwa i lekko ciemniejszymi granicami prowincji. */
  political: Rgba;
  /** Mapa terenu: sam kolor państwa (krycie ustawia renderer). */
  fill: Rgba;
  /** Mapa terenu: flaga państwa rozciągnięta na każdy spójny kawałek terytorium (zachowuje proporcje). */
  flags: Rgba;
  /** Mapa terenu: granica państwa w jego kolorze, `width` kafli w głąb (woda i ląd niczyj też są granicą). */
  borders: Rgba;
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
function flood(tiles: Uint8Array, w: number, h: number, from: number, seen: Uint8Array, order: Int32Array, end: number): number {
  const n = tiles[from];
  seen[from] = 1;
  order[end++] = from;
  for (let q = end - 1; q < end; q++) {
    const j = order[q];
    const x = j % w;
    for (const k of [x > 0 ? j - 1 : -1, x + 1 < w ? j + 1 : -1, j - w, j + w]) {
      if (k >= 0 && k < w * h && !seen[k] && tiles[k] === n) {
        seen[k] = 1;
        order[end++] = k;
      }
    }
  }
  return end;
}

/**
 * Odległość kafla państwa od jego granicy (chamfer 3-4, w trzecich częściach kafla) – granicą jest
 * kafel, którego sąsiad (bok) nie należy do tego państwa (woda, ląd niczyj, inne państwo, brzeg mapy).
 */
function edgeDistance(tiles: Uint8Array, w: number, h: number): Int32Array {
  const INF = 1 << 20;
  const d = new Int32Array(w * h);
  for (let i = 0; i < w * h; i++) {
    if (tiles[i] !== 0) d[i] = isEdge(tiles, w, h, i) ? 0 : INF;
  }
  // Dwa przebiegi (w przód i wstecz) tylko w obrębie tego samego państwa.
  chamfer(tiles, d, w, h, 1);
  chamfer(tiles, d, w, h, -1);
  return d;
}

/** Kafel państwa na granicy: przy brzegu mapy albo z sąsiadem (bok) spoza państwa. */
function isEdge(tiles: Uint8Array, w: number, h: number, i: number): boolean {
  const n = tiles[i];
  const x = i % w;
  if (x === 0 || x + 1 === w || i < w || i + w >= w * h) return true;
  return tiles[i - 1] !== n || tiles[i + 1] !== n || tiles[i - w] !== n || tiles[i + w] !== n;
}

/** Jeden przebieg chamfer 3-4: `dir` = 1 – od lewego górnego rogu (sąsiedzi wcześniejsi), −1 – od prawego dolnego. */
function chamfer(tiles: Uint8Array, d: Int32Array, w: number, h: number, dir: 1 | -1): void {
  const relax = (i: number, j: number, cost: number) => {
    if (tiles[j] === tiles[i] && d[j] + cost < d[i]) d[i] = d[j] + cost;
  };
  const [y0, y1] = dir === 1 ? [1, h - 1] : [h - 2, 0];
  const [x0, x1] = dir === 1 ? [1, w - 1] : [w - 2, 0];
  for (let y = y0; y !== y1; y += dir) {
    for (let x = x0; x !== x1; x += dir) {
      const i = y * w + x;
      if (d[i] === 0 || tiles[i] === 0) continue;
      relax(i, i - dir, 3);
      relax(i, i - dir * w, 3);
      relax(i, i - dir * (w + 1), 4);
      relax(i, i - dir * (w - 1), 4);
    }
  }
}

function put(out: Rgba, i: number, c: Rgb, k = 1, alpha = 255): void {
  const o = i * 4;
  out[o] = c[0] * k;
  out[o + 1] = c[1] * k;
  out[o + 2] = c[2] * k;
  out[o + 3] = alpha;
}

/** Wszystkie warstwy terytoriów z właścicieli kafli (`tiles` – państwo na kafel, 0 = niczyje). */
export function paintTerritories(map: MapPayload, tiles: Uint8Array, borderWidth = TERRITORY_BORDER_WIDTH): TerritoryLayers {
  const { width: w, height: h, province } = map;
  const political = new Uint8ClampedArray(w * h * 4);
  const fill = new Uint8ClampedArray(w * h * 4);
  const flags = new Uint8ClampedArray(w * h * 4);
  const borders = new Uint8ClampedArray(w * h * 4);
  const dist = edgeDistance(tiles, w, h);
  for (let i = 0; i < w * h; i++) {
    const n = tiles[i];
    if (n === 0) continue;
    const color = nationColor(n);
    let k = 1;
    // Na mapie politycznej granicą państwa nie jest woda ani brzeg – tylko inne państwo albo prowincja niczyja.
    if (nationEdge(tiles, province, w, h, i)) k = NATION_EDGE;
    else if (provinceBorder(province, w, h, i)) k = PROVINCE_EDGE;
    put(political, i, color, k);
    put(fill, i, color);
    const depth = dist[i] / 3; // w kaflach
    if (depth < borderWidth) {
      const t = borderWidth > 1 ? depth / (borderWidth - 1) : 0;
      put(borders, i, color, 1, 255 * (1 - (1 - BORDER_INNER) * Math.min(1, t)));
    }
  }
  const parts = pieces(tiles, w, h);
  paintFlags(flags, parts, w);
  return { political, fill, flags, borders, labels: labels(parts, w) };
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
    for (let q = parts.start[p]; q < parts.start[p + 1]; q++) {
      const i = parts.order[q];
      const x = i % w;
      const y = (i - x) / w;
      put(out, i, flagPixel(flag, (x + 0.5 - ox) / (3 * s), (y + 0.5 - oy) / (2 * s)));
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

/** Czy sąsiad kafla (bok) należy do prowincji i ma innego właściciela (także niczyj). */
function nationEdge(tiles: Uint8Array, province: Uint16Array, w: number, h: number, i: number): boolean {
  const n = tiles[i];
  const x = i % w;
  const other = (j: number) => province[j] !== 0 && tiles[j] !== n;
  return (x > 0 && other(i - 1)) || (x + 1 < w && other(i + 1)) || (i >= w && other(i - w)) || (i + w < w * h && other(i + w));
}
