import { Container, Graphics, Text, TextStyle } from 'pixi.js';

import { MIN_SCALE } from './buildings';

/** Jednostka rysunku znaczników = tyle kafli (skaluje się z mapą jak ikony budynków). */
const TILES_PER_UNIT = 0.35;

/** Wygląd znaczników portu (suwak w panelu debugu). */
export interface PortReachStyle {
  /** Promień znacznika portu (px ekranu). */
  ring: number;
}

export const DEFAULT_PORT_REACH_STYLE: PortReachStyle = { ring: 9 };

/** Znaczniki portów: nasz – jasny, partnerzy handlu – złoty. */
const PORT_COLOR = 0xe8f6ff;
const PARTNER_COLOR = 0xf2d47a;

const STYLE = new TextStyle({
  fontFamily: 'system-ui, sans-serif',
  fontWeight: '700',
  fontSize: 11,
  fill: '#ffffff',
  stroke: { color: '#10161d', width: 3 },
});

/** Port do pokazania: kafel portu i porty partnerów z podpisem (koszt dostawy). */
export interface PortView {
  x: number;
  y: number;
  partners: readonly { x: number; y: number; label: string }[];
}

/**
 * Port po kliknięciu ikony portu: znacznik portu i porty partnerów handlu morskiego nad tym samym morzem
 * z kosztem dostawy. Znaczniki mają wielkość w kaflach (jak ikony budynków); z daleka warstwa znika.
 */
export class PortReachLayer {
  readonly view = new Container();
  private style: PortReachStyle = DEFAULT_PORT_REACH_STYLE;
  private current: PortView | null = null;

  /** Port do pokazania (null – nic). */
  set(port: PortView | null): void {
    this.clear();
    this.current = port;
    if (!port) return;
    this.mark(port.x, port.y, PORT_COLOR, '');
    for (const p of port.partners) this.mark(p.x, p.y, PARTNER_COLOR, p.label);
  }

  setStyle(style: PortReachStyle): void {
    this.style = style;
    this.set(this.current);
  }

  /** Co klatkę: z daleka (`scale` – px ekranu na kafel) warstwa znika jak ikony budynków. */
  update(scale: number): void {
    this.view.visible = scale >= MIN_SCALE;
  }

  destroy(): void {
    this.clear();
    this.view.destroy({ children: true });
  }

  private clear(): void {
    for (const c of this.view.removeChildren()) c.destroy({ children: true, texture: true, textureSource: true });
  }

  private mark(x: number, y: number, color: number, label: string): void {
    const view = new Container();
    const r = this.style.ring;
    view.addChild(new Graphics().circle(0, 0, r).stroke({ width: 2.5, color: 0x10161d }).circle(0, 0, r).stroke({ width: 1.5, color }));
    if (label) {
      const text = new Text({ text: label, style: STYLE, resolution: 6 });
      text.anchor.set(0.5, 1);
      text.position.set(0, -r - 2);
      view.addChild(text);
    }
    view.position.set(x + 0.5, y + 0.5);
    view.scale.set(TILES_PER_UNIT);
    this.view.addChild(view);
  }
}
