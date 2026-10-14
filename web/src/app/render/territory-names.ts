import { Container, Text, TextStyle } from 'pixi.js';

import type { TerritoryLabel } from './owners';

/** Wielkość liter względem grubości terytorium i odstępu między literami. */
const SIZE_OF_THICKNESS = 0.5;
const SIZE_OF_SPACING = 0.9;
/** Najmniejsza i największa litera (kafle). */
const MIN_SIZE = 6;
const MAX_SIZE = 220;
/**
 * Widoczność zależnie od wielkości litery na ekranie (px): poniżej `FADE_IN_FROM` niewidoczna,
 * pełna od `FADE_IN_TO`, znika od `FADE_OUT_FROM` do `FADE_OUT_TO` (przy dużym przybliżeniu).
 */
const FADE_IN_FROM = 5;
const FADE_IN_TO = 9;
const FADE_OUT_FROM = 60;
const FADE_OUT_TO = 110;

const STYLE = new TextStyle({
  fontFamily: 'Georgia, "Times New Roman", serif',
  fontWeight: '600',
  fill: '#1b1814',
  // Litery rysowane raz w dużej rozdzielczości i skalowane – ostre przy każdym przybliżeniu, w którym są widoczne.
  fontSize: 96,
});

interface Placed {
  view: Container;
  /** Wysokość litery w kaflach. */
  size: number;
}

/**
 * Nazwy państw na mapie (jak w EU4): wielkie litery rozłożone wzdłuż wygiętej osi największego
 * kawałka terytorium, wielkość według rozmiaru państwa. Znikają przy dużym przybliżeniu.
 */
export class TerritoryNames {
  readonly view = new Container();
  private placed: Placed[] = [];
  private opacity = 0.8;
  private labels: TerritoryLabel[] = [];
  private names = new Map<number, string>();

  /** Położenie nazw z workera malującego (`paintTerritories`) i nazwy państw (numer → nazwa). */
  set(labels: TerritoryLabel[], names: Map<number, string>): void {
    this.labels = labels;
    this.names = names;
    this.build();
  }

  setNames(names: Map<number, string>): void {
    if (sameNames(names, this.names)) return;
    this.names = names;
    this.build();
  }

  setOpacity(opacity: number): void {
    this.opacity = opacity;
  }

  /** Krycie każdej nazwy według wielkości liter na ekranie (`scale` – piksele na kafel). */
  update(scale: number): void {
    for (const p of this.placed) {
      const px = p.size * scale;
      const fadeIn = smooth((px - FADE_IN_FROM) / (FADE_IN_TO - FADE_IN_FROM));
      const fadeOut = 1 - smooth((px - FADE_OUT_FROM) / (FADE_OUT_TO - FADE_OUT_FROM));
      const alpha = this.opacity * fadeIn * fadeOut;
      p.view.alpha = alpha;
      p.view.visible = alpha > 0.01;
    }
  }

  clear(): void {
    for (const p of this.placed) p.view.destroy({ children: true });
    this.placed = [];
  }

  destroy(): void {
    this.clear();
    this.view.destroy();
  }

  private build(): void {
    this.clear();
    for (const label of this.labels) {
      const name = this.names.get(label.nation);
      if (!name) continue;
      const placed = place(label, name.toUpperCase());
      this.view.addChild(placed.view);
      this.placed.push(placed);
    }
  }
}

/** Litery napisu wzdłuż krzywej etykiety, równo rozłożone na całej długości. */
function place(label: TerritoryLabel, text: string): Placed {
  const chars = [...text];
  const length = label.u1 - label.u0;
  const spacing = length / Math.max(1, chars.length);
  const size = Math.min(MAX_SIZE, Math.max(MIN_SIZE, Math.min(label.thickness * SIZE_OF_THICKNESS, spacing * SIZE_OF_SPACING)));
  // Krótka nazwa w długim państwie – litery nie rozjeżdżają się dalej niż na ~2,2 wysokości.
  const step = Math.min(spacing, size * 2.2);
  const from = (label.u0 + label.u1) / 2 - (step * chars.length) / 2;
  const cos = Math.cos(label.angle);
  const sin = Math.sin(label.angle);
  const view = new Container();
  chars.forEach((ch, k) => {
    if (ch.trim() === '') return;
    const u = from + step * (k + 0.5);
    const v = label.a * u * u + label.b * u + label.c;
    const letter = new Text({ text: ch, style: STYLE, resolution: 1 });
    letter.anchor.set(0.5);
    letter.scale.set(size / STYLE.fontSize);
    letter.position.set(label.cx + u * cos - v * sin, label.cy + u * sin + v * cos);
    letter.rotation = label.angle + Math.atan(2 * label.a * u + label.b);
    view.addChild(letter);
  });
  return { view, size };
}

function smooth(t: number): number {
  const x = Math.min(1, Math.max(0, t));
  return x * x * (3 - 2 * x);
}

function sameNames(a: Map<number, string>, b: Map<number, string>): boolean {
  return a.size === b.size && [...a].every(([k, v]) => b.get(k) === v);
}
