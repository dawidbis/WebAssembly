import { Container, Graphics, Sprite, Text, TextStyle, Texture } from 'pixi.js';

import type { SailingReport } from '../../generated/SailingReport';
import { short } from '../ui/format';
import { flagCanvas } from './flags';
import { MIN_SCALE } from './buildings';
import { nationColor } from './nations';

/** Tura gry w ms (`core/src/protocol.rs`) – kropka sunie płynnie między turami. */
const TICK_MS = 100;
/** Zaokrętowanie przed rejsem (`war::EMBARK_TICKS`): pierścień ładowania wojska na statki przy porcie. */
const EMBARK_TICKS = 20;
const EMBARK_COLOR = 0xf2d47a;
/**
 * Wielkość na mapie: jednostka rysunku (kropka, flaga, napis, linia) = tyle kafli – skaluje się z mapą jak
 * ikony budynków (flaga ≈ szerokość ikony budynku); z daleka warstwa znika (`MIN_SCALE`, jak budynki).
 */
const TILES_PER_UNIT = 0.35;
/** Rozdzielczość tekstur flagi i napisu (ostre przy dużym przybliżeniu). */
const SHARP = 6;
/** Miniaturka flagi nad kropką (jednostki rysunku). */
const FLAG_W = 18;
const FLAG_H = 12;

/** Wygląd desantów na mapie (suwaki w panelu debugu). */
export interface LandingStyle {
  /** Promień kropki (px ekranu). */
  dot: number;
  /** Skala napisu z flagą i liczbą wojska. */
  label: number;
  /** Krycie linii do celu (0 – bez linii). */
  route: number;
}

export const DEFAULT_LANDING_STYLE: LandingStyle = { dot: 5, label: 1, route: 0.6 };

const STYLE = new TextStyle({
  fontFamily: 'system-ui, sans-serif',
  fontWeight: '700',
  fontSize: 12,
  fill: '#ffffff',
  stroke: { color: '#10161d', width: 3 },
});

interface Marker {
  report: SailingReport;
  view: Container;
  dot: Graphics;
  label: Container;
  /** Pierścień zaokrętowania, napis z liczbą wojska i ostatnio pokazana liczba. */
  ring: Graphics;
  text: Text;
  shown: string;
  /** Droga rejsu (środki kafli) i długość od początku do każdego punktu. */
  points: readonly [number, number][];
  lengths: readonly number[];
}

/**
 * Płynące desanty: kropka w kolorze państwa sunie od portu do brzegu celu po drodze rejsu z rdzenia
 * (najkrótsza droga po kaflach morza), nad nią flaga i liczba wojska, dalsza droga – przerywana linia.
 * Przed wypłynięciem przy porcie pierścień zaokrętowania z rosnącą liczbą wojska.
 * Wielkość w kaflach (jak ikony budynków), z daleka niewidoczne.
 */
export class LandingLayer {
  readonly view = new Container();
  private readonly routes = new Graphics();
  private markers: Marker[] = [];
  private readonly flags = new Map<number, Texture>();
  private style: LandingStyle = DEFAULT_LANDING_STYLE;
  /** Ostatnia tura gry i chwila, w której przyszła (interpolacja między turami). */
  private tick = 0;
  private tickAt = 0;

  constructor() {
    this.view.addChild(this.routes);
  }

  /** Desanty z bieżącej tury (`landings` z workera) i numer tury. */
  set(list: readonly SailingReport[], tick: number): void {
    if (tick !== this.tick) {
      this.tick = tick;
      this.tickAt = performance.now();
    }
    const same =
      list.length === this.markers.length && list.every((r, i) => sameReport(r, this.markers[i].report));
    if (same) return;
    for (const m of this.markers) m.view.destroy({ children: true });
    this.markers = list.map((r) => this.marker(r));
  }

  setStyle(style: LandingStyle): void {
    this.style = style;
    const reports = this.markers.map((m) => m.report);
    for (const m of this.markers) m.view.destroy({ children: true });
    this.markers = reports.map((r) => this.marker(r));
  }

  /** Co klatkę: położenie kropek i linie do celu; `scale` – skala świata (px ekranu na kafel). */
  update(scale: number): void {
    this.routes.clear();
    this.view.visible = scale >= MIN_SCALE;
    if (this.markers.length === 0 || !this.view.visible) return;
    const now = this.tick + Math.min(1, (performance.now() - this.tickAt) / TICK_MS);
    const inv = TILES_PER_UNIT;
    for (const m of this.markers) {
      const r = m.report;
      const p = r.arrive > r.depart ? Math.min(1, Math.max(0, (now - r.depart) / (r.arrive - r.depart))) : 1;
      const { x, y, k } = along(m, p);
      m.view.position.set(x, y);
      m.view.scale.set(inv);
      this.embark(m, now);
      if (this.style.route > 0) {
        const rest: [number, number][] = [[x, y], ...m.points.slice(k + 1), [r.toX + 0.5, r.toY + 0.5]];
        dashed(this.routes, rest, inv, nationColor(r.nation), this.style.route);
      }
    }
  }

  /**
   * Zaokrętowanie (przed wypłynięciem): pierścień wokół kropki napełnia się, a liczba wojska rośnie od zera
   * do całego desantu; potem pierścień znika i liczba jest stała.
   */
  private embark(m: Marker, now: number): void {
    const r = m.report;
    const p = Math.min(1, Math.max(0, (now - (r.depart - EMBARK_TICKS)) / EMBARK_TICKS));
    const loading = p < 1;
    m.ring.visible = loading;
    if (loading) {
      const R = this.style.dot + 4;
      m.ring
        .clear()
        .circle(0, 0, R)
        .stroke({ width: 3, color: 0x10161d, alpha: 0.7 })
        .arc(0, 0, R, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * Math.max(0.001, p))
        .stroke({ width: 2.5, color: EMBARK_COLOR, cap: 'round' });
    }
    const shown = short(loading ? r.troops * p : r.troops);
    if (shown !== m.shown) {
      m.shown = shown;
      m.text.text = shown;
    }
  }

  destroy(): void {
    this.view.destroy({ children: true });
    for (const t of this.flags.values()) t.destroy(true);
    this.flags.clear();
  }

  private marker(report: SailingReport): Marker {
    const view = new Container();
    const [r, g, b] = nationColor(report.nation);
    const dot = new Graphics()
      .circle(0, 0, this.style.dot)
      .fill({ color: (r << 16) | (g << 8) | b })
      .stroke({ width: 1.5, color: 0xffffff });
    const label = new Container();
    const flag = new Sprite(this.flag(report.nation));
    flag.width = FLAG_W;
    flag.height = FLAG_H;
    const frame = new Graphics().rect(-0.5, -0.5, FLAG_W + 1, FLAG_H + 1).stroke({ width: 1, color: 0x10161d });
    const text = new Text({ text: short(report.troops), style: STYLE, resolution: SHARP });
    text.position.set(FLAG_W + 4, (FLAG_H - text.height) / 2);
    label.addChild(flag, frame, text);
    label.scale.set(this.style.label);
    label.pivot.set((FLAG_W + 4 + text.width) / 2, FLAG_H);
    label.position.set(0, -this.style.dot - 4);
    const ring = new Graphics();
    view.addChild(ring, dot, label);
    this.view.addChild(view);
    // Droga: port, punkty rejsu po morzu (środki kafli); bez drogi – prosto do celu.
    const points: [number, number][] = [[report.fromX + 0.5, report.fromY + 0.5], ...report.path.map(([x, y]) => [x + 0.5, y + 0.5] as [number, number])];
    const lengths = [0];
    for (let k = 1; k < points.length; k++) {
      lengths.push(lengths[k - 1] + Math.hypot(points[k][0] - points[k - 1][0], points[k][1] - points[k - 1][1]));
    }
    return { report, view, dot, label, ring, text, shown: text.text, points, lengths };
  }

  private flag(nation: number): Texture {
    let t = this.flags.get(nation);
    if (!t) {
      const canvas = flagCanvas(nation, FLAG_W * SHARP, FLAG_H * SHARP);
      t = canvas ? Texture.from(canvas) : Texture.WHITE;
      this.flags.set(nation, t);
    }
    return t;
  }
}

function sameReport(a: SailingReport, b: SailingReport): boolean {
  return (
    a.nation === b.nation &&
    a.troops === b.troops &&
    a.depart === b.depart &&
    a.arrive === b.arrive &&
    a.fromX === b.fromX &&
    a.fromY === b.fromY &&
    a.toX === b.toX &&
    a.toY === b.toY &&
    a.path.length === b.path.length
  );
}

/** Punkt na drodze rejsu w części `p` (0–1) jej długości i numer odcinka, na którym leży. */
function along(m: Marker, p: number): { x: number; y: number; k: number } {
  const { points, lengths } = m;
  const total = lengths[lengths.length - 1];
  if (points.length < 2 || total <= 0) return { x: points[0][0], y: points[0][1], k: 0 };
  const at = p * total;
  let k = 0;
  while (k < lengths.length - 2 && lengths[k + 1] < at) k++;
  const span = lengths[k + 1] - lengths[k];
  const t = span > 0 ? (at - lengths[k]) / span : 0;
  return { x: points[k][0] + (points[k + 1][0] - points[k][0]) * t, y: points[k][1] + (points[k + 1][1] - points[k][1]) * t, k };
}

/** Przerywana linia po punktach (kreski stałej długości na ekranie; `px` – kafle na piksel ekranu). */
function dashed(g: Graphics, points: readonly [number, number][], px: number, rgb: readonly number[], alpha: number): void {
  const dash = 6 * px;
  const gap = 4 * px;
  // Faza kreski przechodzi przez kolejne odcinki – linia wygląda jednolicie na zakrętach.
  let phase = 0;
  for (let k = 1; k < points.length; k++) {
    const [x0, y0] = points[k - 1];
    const [x1, y1] = points[k];
    const len = Math.hypot(x1 - x0, y1 - y0);
    if (len <= 0) continue;
    const dx = (x1 - x0) / len;
    const dy = (y1 - y0) / len;
    let s = -phase;
    while (s < len) {
      const a = Math.max(0, s);
      const e = Math.min(len, s + dash);
      if (e > a) g.moveTo(x0 + dx * a, y0 + dy * a).lineTo(x0 + dx * e, y0 + dy * e);
      s += dash + gap;
    }
    phase = (phase + len) % (dash + gap);
  }
  g.stroke({ width: 1.5 * px, color: (rgb[0] << 16) | (rgb[1] << 8) | rgb[2], alpha });
}
