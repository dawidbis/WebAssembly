import { Container, Graphics, Sprite, Text, TextStyle, Texture } from 'pixi.js';

import type { FrontMark } from '../../generated/FrontMark';
import { short } from '../ui/format';
import { flagCanvas } from './flags';

/** Rozdzielczość tekstur flagi i napisu (ostre na ekranach o dużej gęstości). */
const SHARP = 3;
/** Miniaturka flagi atakującego (px ekranu). */
const FLAG_W = 15;
const FLAG_H = 10;
/** Znacznik dojeżdża do nowego miejsca na linii frontu: część drogi na klatkę (60 fps). */
const FOLLOW = 0.15;
/** Znacznik „ten sam” po turze, gdy przesunął się najwyżej o tyle kafli (inaczej nowy). */
const SAME_TILES = 40;

const STYLE = new TextStyle({
  fontFamily: 'system-ui, sans-serif',
  fontWeight: '700',
  fontSize: 11,
  fill: '#ffffff',
  stroke: { color: '#10161d', width: 3 },
});

interface Marker {
  mark: FrontMark;
  view: Container;
  text: Text;
  back: Graphics;
  /** Położenie na ekranie (kafle) – dojeżdża do `mark`. */
  x: number;
  y: number;
}

/**
 * Walczące fronty: przy linii frontu miniaturka flagi atakującego i „wojsko vs obrońcy” (obrońcy – państwo
 * w kraju razem z przeciwnym frontem albo tubylcy). Stała wielkość na ekranie; opcja pod zębatką
 * (`TerritoryStyle.showFronts`) chowa całą warstwę.
 */
export class FrontMarkLayer {
  readonly view = new Container();
  private markers: Marker[] = [];
  private readonly flags = new Map<number, Texture>();

  /** Znaczniki z bieżącej tury (`frontMarks` z workera). */
  set(list: readonly FrontMark[]): void {
    const old = this.markers;
    const next: Marker[] = [];
    for (const mark of list) {
      // Ten sam front: to samo państwo i obrońca, najbliższy z poprzedniej tury.
      let best = -1;
      let dist = SAME_TILES * SAME_TILES;
      old.forEach((m, k) => {
        if (m.mark.nation !== mark.nation || m.mark.enemy !== mark.enemy) return;
        const d = (m.mark.x - mark.x) ** 2 + (m.mark.y - mark.y) ** 2;
        if (d <= dist) {
          dist = d;
          best = k;
        }
      });
      if (best >= 0) {
        const m = old.splice(best, 1)[0];
        if (m.mark.troops !== mark.troops || m.mark.enemyTroops !== mark.enemyTroops) this.label(m, mark);
        m.mark = mark;
        next.push(m);
      } else {
        next.push(this.marker(mark));
      }
    }
    for (const m of old) m.view.destroy({ children: true });
    this.markers = next;
  }

  /** Co klatkę: dojazd do miejsca na froncie, stała wielkość na ekranie (`scale` – px ekranu na kafel). */
  update(scale: number): void {
    if (!this.view.visible) return;
    const inv = 1 / scale;
    for (const m of this.markers) {
      m.x += (m.mark.x + 0.5 - m.x) * FOLLOW;
      m.y += (m.mark.y + 0.5 - m.y) * FOLLOW;
      m.view.position.set(m.x, m.y);
      m.view.scale.set(inv);
    }
  }

  destroy(): void {
    this.view.destroy({ children: true });
    for (const t of this.flags.values()) t.destroy(true);
    this.flags.clear();
  }

  private marker(mark: FrontMark): Marker {
    const view = new Container();
    const back = new Graphics();
    const flag = new Sprite(this.flag(mark.nation));
    flag.width = FLAG_W;
    flag.height = FLAG_H;
    const frame = new Graphics().rect(-0.5, -0.5, FLAG_W + 1, FLAG_H + 1).stroke({ width: 1, color: 0x10161d });
    const text = new Text({ text: '', style: STYLE, resolution: SHARP });
    view.addChild(back, flag, frame, text);
    this.view.addChild(view);
    const m: Marker = { mark, view, text, back, x: mark.x + 0.5, y: mark.y + 0.5 };
    this.label(m, mark);
    return m;
  }

  /** Napis „ile vs ile” i tło pod całością; środek znacznika nad kaflem frontu. */
  private label(m: Marker, mark: FrontMark): void {
    m.text.text = `${short(mark.troops)} vs ${short(mark.enemyTroops)}`;
    m.text.position.set(FLAG_W + 4, (FLAG_H - m.text.height) / 2);
    const w = FLAG_W + 4 + m.text.width;
    m.back.clear().roundRect(-3, -3, w + 6, FLAG_H + 6, 3).fill({ color: 0x10161d, alpha: 0.55 });
    m.view.pivot.set(w / 2, FLAG_H / 2);
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
