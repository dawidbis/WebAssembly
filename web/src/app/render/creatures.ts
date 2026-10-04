import { Container, Graphics } from 'pixi.js';

import type { MapPayload } from '../worker/protocol';

/**
 * Yeti (pikselowy, 11 × 12) – easter egg w enklawach (dolinach odciętych dalej, niż sięga
 * najdłuższy tunel). Znaki: o – obrys, W – futro, G – cień futra, F – twarz, E – oko, M – pysk.
 */
const YETI = [
  '....ooo....',
  '..ooWWWoo..',
  '.oWWWWWWWo.',
  '.oWFFFFFWo.',
  '.oWFEFEFWo.',
  '.oWFFFFFWo.',
  'ooWWFMFWWoo',
  'oWWWWWWWWWo',
  'oGGWWWWWGGo',
  '.ooGWWWGoo.',
  '..oGWoWGo..',
  '..ooo.ooo..',
];
const COLORS: Record<string, number> = { o: 0x2e3d4f, W: 0xf4f8fb, G: 0xc6d3df, F: 0x8fa7bd, E: 0x14181c, M: 0x4a2633 };

/** Domyślny rozmiar „piksela” yeti w kaflach (suwak „Wielkość yeti” w panelu debugu). */
export const YETI_PIXEL = 0.5;

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

/** Yeti na środkach enklaw – nad terenem i granicami, pod podświetleniem. */
export class CreatureLayer {
  readonly view = new Container();
  /** Rozmiar „piksela” yeti w kaflach. */
  pixel = YETI_PIXEL;

  /** Rysuje yeti w enklawach z „ostatnich szlifów”; zwraca ich liczbę. */
  setMap(map: MapPayload): number {
    this.clear();
    let count = 0;
    for (const p of map.enclaves) {
      const s = this.pixel;
      const g = new Graphics();
      YETI.forEach((row, y) => {
        for (let x = 0; x < row.length; x++) {
          const color = COLORS[row[x]];
          if (color !== undefined) g.rect(x * s, y * s, s, s).fill(color);
        }
      });
      // Na środku enklawy, ale w granicach mapy (enklawy bywają przy krawędzi).
      const w = YETI[0].length * s;
      const h = YETI.length * s;
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
