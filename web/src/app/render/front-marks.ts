import { Container, Graphics, Sprite, Text, TextStyle, Texture } from 'pixi.js';

import type { FrontMark } from '../../generated/FrontMark';
import { short } from '../ui/format';
import { MIN_SCALE } from './buildings';
import { flagCanvas } from './flags';

/** Rozdzielczość tekstur flagi i napisu (ostre na ekranach o dużej gęstości). */
const SHARP = 3;
/** Miniaturka flagi (px ekranu). */
const FLAG_W = 15;
const FLAG_H = 10;
/** Odstęp flagi od napisu (px). */
const GAP = 4;
/** Znacznik dojeżdża do nowego miejsca na linii frontu: część drogi na klatkę (60 fps). */
const FOLLOW = 0.15;
/** Znacznik „ten sam” po turze, gdy przesunął się najwyżej o tyle kafli (inaczej nowy). */
const SAME_TILES = 40;
/** Tubylcy: chorągiewka w kolorze ziemi niczyjej. */
const NATIVES_COLOR = 0x8d8676;

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
  /** Flaga obrońcy (zmienia się, gdy zmieni się główny obrońca). */
  enemy: Container;
  /** Położenie (kafle) – dojeżdża do `mark`. */
  x: number;
  y: number;
}

/**
 * Walczące fronty: przy linii frontu flaga atakującego, „wojsko vs obrońcy” i flaga obrońcy (państwo – w kraju
 * albo w przyczółku razem z przeciwnym frontem; tubylcy – szara chorągiewka). Stała wielkość na ekranie,
 * z daleka znika jak ikony budynków (`MIN_SCALE`); opcja pod zębatką (`TerritoryStyle.showFronts`).
 */
export class FrontMarkLayer {
  readonly view = new Container();
  private markers: Marker[] = [];
  private readonly flags = new Map<number, Texture>();
  private enabled = true;

  /** Znaczniki z bieżącej tury (`frontMarks` z workera). */
  set(list: readonly FrontMark[]): void {
    const old = this.markers;
    const next: Marker[] = [];
    for (const mark of list) {
      // Ten sam front: to samo państwo atakujące, najbliższy z poprzedniej tury.
      let best = -1;
      let dist = SAME_TILES * SAME_TILES;
      old.forEach((m, k) => {
        if (m.mark.nation !== mark.nation) return;
        const d = (m.mark.x - mark.x) ** 2 + (m.mark.y - mark.y) ** 2;
        if (d <= dist) {
          dist = d;
          best = k;
        }
      });
      if (best >= 0) {
        const m = old.splice(best, 1)[0];
        const enemy = m.mark.enemy !== mark.enemy;
        if (enemy || m.mark.troops !== mark.troops || m.mark.enemyTroops !== mark.enemyTroops) this.label(m, mark, enemy);
        m.mark = mark;
        next.push(m);
      } else {
        next.push(this.marker(mark));
      }
    }
    for (const m of old) m.view.destroy({ children: true });
    this.markers = next;
  }

  /** Opcja pod zębatką: pokazuj znaczniki. */
  setEnabled(enabled: boolean): void {
    this.enabled = enabled;
  }

  /** Co klatkę: dojazd do miejsca na froncie, stała wielkość na ekranie (`scale` – px ekranu na kafel). */
  update(scale: number): void {
    this.view.visible = this.enabled && scale >= MIN_SCALE;
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
    const text = new Text({ text: '', style: STYLE, resolution: SHARP });
    const enemy = new Container();
    view.addChild(back, this.flagView(mark.nation), text, enemy);
    this.view.addChild(view);
    const m: Marker = { mark, view, text, back, enemy, x: mark.x + 0.5, y: mark.y + 0.5 };
    this.label(m, mark, true);
    return m;
  }

  /** Napis „ile vs ile”, flaga obrońcy i tło pod całością; środek znacznika nad kaflem frontu. */
  private label(m: Marker, mark: FrontMark, enemy: boolean): void {
    m.text.text = `${short(mark.troops)} vs ${short(mark.enemyTroops)}`;
    m.text.position.set(FLAG_W + GAP, (FLAG_H - m.text.height) / 2);
    const right = FLAG_W + GAP + m.text.width + GAP;
    if (enemy) {
      for (const c of m.enemy.removeChildren()) c.destroy({ children: true });
      m.enemy.addChild(mark.enemy > 0 ? this.flagView(mark.enemy) : nativesFlag());
    }
    m.enemy.position.set(right, 0);
    const w = right + FLAG_W;
    m.back.clear().roundRect(-3, -3, w + 6, FLAG_H + 6, 3).fill({ color: 0x10161d, alpha: 0.55 });
    m.view.pivot.set(w / 2, FLAG_H / 2);
  }

  /** Flaga państwa z ramką. */
  private flagView(nation: number): Container {
    const c = new Container();
    const flag = new Sprite(this.flag(nation));
    flag.width = FLAG_W;
    flag.height = FLAG_H;
    c.addChild(flag, new Graphics().rect(-0.5, -0.5, FLAG_W + 1, FLAG_H + 1).stroke({ width: 1, color: 0x10161d }));
    return c;
  }

  private flag(nation: number): Texture {
    let t = this.flags.get(nation);
    if (!t || t.destroyed) {
      const canvas = flagCanvas(nation, FLAG_W * SHARP, FLAG_H * SHARP);
      t = canvas ? Texture.from(canvas) : Texture.WHITE;
      this.flags.set(nation, t);
    }
    return t;
  }
}

/** Tubylcy: szara chorągiewka z jasnym trójkątem (namiot). */
function nativesFlag(): Graphics {
  return new Graphics()
    .rect(0, 0, FLAG_W, FLAG_H)
    .fill({ color: NATIVES_COLOR })
    .poly([FLAG_W / 2, 2, FLAG_W / 2 + 4, FLAG_H - 2, FLAG_W / 2 - 4, FLAG_H - 2])
    .fill({ color: 0xe8e1d0 })
    .rect(-0.5, -0.5, FLAG_W + 1, FLAG_H + 1)
    .stroke({ width: 1, color: 0x10161d });
}
