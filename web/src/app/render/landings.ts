import { Container, Graphics, Sprite, Text, TextStyle, Texture } from 'pixi.js';

import type { SailingReport } from '../../generated/SailingReport';
import { short } from '../ui/format';
import { flagCanvas } from './flags';
import { nationColor } from './nations';

/** Tura gry w ms (`core/src/protocol.rs`) – kropka sunie płynnie między turami. */
const TICK_MS = 100;
/** Miniaturka flagi nad kropką (px ekranu). */
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
}

/**
 * Płynące desanty: kropka w kolorze państwa sunie od portu do brzegu celu (w linii prostej, jak liczy
 * rejs rdzeń), nad nią flaga i liczba wojska, do celu przerywana linia. Wielkość stała na ekranie.
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
    if (this.markers.length === 0) return;
    const now = this.tick + Math.min(1, (performance.now() - this.tickAt) / TICK_MS);
    const inv = 1 / scale;
    for (const m of this.markers) {
      const r = m.report;
      const p = r.arrive > r.depart ? Math.min(1, Math.max(0, (now - r.depart) / (r.arrive - r.depart))) : 1;
      const x = r.fromX + 0.5 + (r.toX - r.fromX) * p;
      const y = r.fromY + 0.5 + (r.toY - r.fromY) * p;
      m.view.position.set(x, y);
      m.view.scale.set(inv);
      if (this.style.route > 0) dashed(this.routes, x, y, r.toX + 0.5, r.toY + 0.5, inv, nationColor(r.nation), this.style.route);
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
    const text = new Text({ text: short(report.troops), style: STYLE });
    text.position.set(FLAG_W + 4, (FLAG_H - text.height) / 2);
    label.addChild(flag, frame, text);
    label.scale.set(this.style.label);
    label.pivot.set((FLAG_W + 4 + text.width) / 2, FLAG_H);
    label.position.set(0, -this.style.dot - 4);
    view.addChild(dot, label);
    this.view.addChild(view);
    return { report, view, dot, label };
  }

  private flag(nation: number): Texture {
    let t = this.flags.get(nation);
    if (!t) {
      const canvas = flagCanvas(nation, FLAG_W * 2, FLAG_H * 2);
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
    a.toY === b.toY
  );
}

/** Przerywana linia (kreski stałej długości na ekranie; `px` – kafle na piksel ekranu). */
function dashed(g: Graphics, x0: number, y0: number, x1: number, y1: number, px: number, rgb: readonly number[], alpha: number): void {
  const len = Math.hypot(x1 - x0, y1 - y0);
  if (len <= 0) return;
  const dash = 6 * px;
  const gap = 4 * px;
  const dx = (x1 - x0) / len;
  const dy = (y1 - y0) / len;
  for (let s = 0; s < len; s += dash + gap) {
    const e = Math.min(len, s + dash);
    g.moveTo(x0 + dx * s, y0 + dy * s).lineTo(x0 + dx * e, y0 + dy * e);
  }
  g.stroke({ width: 1.5 * px, color: (rgb[0] << 16) | (rgb[1] << 8) | rgb[2], alpha });
}
