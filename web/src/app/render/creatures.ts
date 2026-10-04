import { Container, Graphics } from 'pixi.js';

import type { MapPayload } from '../worker/protocol';

/**
 * Stworek (pikselowy, 12 × 13) – easter egg w prowincjach zablokowanych (bez sąsiadów i bez
 * dostępu do oceanu). Znaki: o – obrys, B – futro, W – białko oka, P – źrenica, T – ząbki.
 */
const CREATURE = [
  '.oo......oo.',
  '..oooooooo..',
  '..oBBBBBBo..',
  '.oBBBBBBBBo.',
  'oBBWWBBWWBBo',
  'oBBWPBBWPBBo',
  'oBBBBBBBBBBo',
  'oBBooooooBBo',
  'oBBoTooToBBo',
  'oBBBBBBBBBBo',
  '.oBBBBBBBBo.',
  '.oBBo..oBBo.',
  '.ooo....ooo.',
];
const COLORS: Record<string, number> = { o: 0x2a1d3a, B: 0x8a6bd1, W: 0xffffff, P: 0x1a1a1a, T: 0xffffff };

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

/** Stworki na środkach zablokowanych prowincji – nad terenem i granicami, pod podświetleniem. */
export class CreatureLayer {
  readonly view = new Container();

  /** Rysuje stworki dla prowincji oznaczonych przez „ostatnie szlify”; zwraca ich liczbę. */
  setMap(map: MapPayload): number {
    this.clear();
    let count = 0;
    for (const p of map.provinces) {
      if (!p.blocked) continue;
      // Rozmiar „piksela” stworka w kaflach – zależny od wielkości prowincji.
      const s = Math.max(1, Math.min(3, Math.round(Math.sqrt(p.area) / 12)));
      const g = new Graphics();
      CREATURE.forEach((row, y) => {
        for (let x = 0; x < row.length; x++) {
          const color = COLORS[row[x]];
          if (color !== undefined) g.rect(x * s, y * s, s, s).fill(color);
        }
      });
      // Na środku prowincji, ale w granicach mapy (prowincje zablokowane bywają przy krawędzi).
      const w = CREATURE[0].length * s;
      const h = CREATURE.length * s;
      g.position.set(clamp(p.centerX + 0.5 - w / 2, 0, map.width - w), clamp(p.centerY + 0.5 - h / 2, 0, map.height - h));
      this.view.addChild(g);
      count++;
    }
    return count;
  }

  clear(): void {
    for (const old of this.view.removeChildren()) old.destroy();
  }

  destroy(): void {
    this.clear();
    this.view.destroy();
  }
}
