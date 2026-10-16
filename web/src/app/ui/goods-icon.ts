import { Component, computed, input } from '@angular/core';

import type { Supply } from '../../generated/Supply';
import { ICONS, RESOURCES, type IconShape } from '../render/resources';

/** Ikony rodzajów zaopatrzenia (24 × 24, obrys `currentColor`) i ich kolory. */
export const SUPPLY_ICONS: Record<Supply, { name: string; color: string; d: string }> = {
  lightMilitary: { name: 'Lekkie wojskowe', color: '#7d8f5a', d: 'M10 21V9l2-5 2 5v12zM10 12h4' },
  standardMilitary: { name: 'Standardowe wojskowe', color: '#5f7d3c', d: 'M6 21V10l2-5 2 5v11zM14 21V10l2-5 2 5v11z' },
  heavyMilitary: { name: 'Ciężkie wojskowe', color: '#4d5b34', d: 'M3 16h18v4H3zM6 16v-4h9v4M15 13l6-2M8 20h0M12 20h0M16 20h0' },
  fuel: { name: 'Paliwo', color: '#a8452c', d: 'M6 5h9l3 3v13H6zM9 5V3h4M9 11h6M9 15h6' },
  basicCivil: { name: 'Podstawowe cywilne', color: '#b0814a', d: 'M4 13c0-4 3.6-7 8-7s8 3 8 7v6H4zM8 9v3M12 8v4M16 9v3' },
  luxuryCivil: { name: 'Luksusowe cywilne', color: '#9a4f9b', d: 'M6 4h12l3 5-9 11L3 9zM3 9h18M9 4l3 16 3-16' },
  engineeringMaterials: { name: 'Materiały inżynieryjne', color: '#6f7f8e', d: 'M3 20h8v-4H3zM13 20h8v-4h-8zM8 15h8v-4H8zM14.5 3.5l3 3-6.5 6.5-3-3z' },
  provisions: { name: 'Prowiant', color: '#b9a53a', d: 'M12 21V8M12 8c-2-.5-3-2-3-4 2 0 3 1.6 3 4zm0 0c2-.5 3-2 3-4-2 0-3 1.6-3 4zM12 13c-2-.5-3-2-3-4 2 0 3 1.6 3 4zm0 0c2-.5 3-2 3-4-2 0-3 1.6-3 4zM12 18c-2-.5-3-2-3-4 2 0 3 1.6 3 4zm0 0c2-.5 3-2 3-4-2 0-3 1.6-3 4z' },
};

/**
 * Ikona dobra: surowca (kolor z mapy zasobów; energia – ikona, rolnictwo, hodowla i las – piktogram,
 * metale i chemia – symbol) albo zaopatrzenia (ikona rodzaju). Nazwa w podpowiedzi.
 */
@Component({
  selector: 'app-goods-icon',
  template: `
    <span class="tile" [style.background]="v().color" [style.width.px]="size()" [style.height.px]="size()" [title]="v().name" [attr.aria-label]="v().name">
      @if (v().shapes; as shapes) {
        <svg viewBox="0 0 24 24" aria-hidden="true">
          @for (s of shapes; track $index) {
            <path [attr.d]="s.d" [attr.fill]="s.fill" [attr.stroke]="s.stroke ?? null" [attr.stroke-width]="s.strokeWidth ?? null" />
          }
        </svg>
      } @else if (v().glyph) {
        <svg viewBox="0 0 24 24" aria-hidden="true" class="glyph"><path class="under" [attr.d]="v().glyph" /><path [attr.d]="v().glyph" /></svg>
      } @else if (v().path) {
        <svg viewBox="0 0 24 24" aria-hidden="true" class="line"><path [attr.d]="v().path" /></svg>
      } @else {
        <span class="text">{{ v().text }}</span>
      }
    </span>
  `,
  styles: `
    :host { display: inline-flex; }
    .tile { display: grid; place-items: center; flex: none; border-radius: 4px; box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.35); }
    svg { width: 78%; height: 78%; }
    svg.line path { fill: none; stroke: #fff; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
    svg.glyph path { fill: none; stroke: #fff; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
    svg.glyph path.under { stroke: #000; stroke-width: 4.2; }
    .text { font-size: 9px; font-weight: 700; line-height: 1; color: #fff; -webkit-text-stroke: 2px rgb(0 0 0 / 0.75); paint-order: stroke fill; }
  `,
})
export class GoodsIcon {
  /** Numer surowca albo rodzaj zaopatrzenia. */
  readonly resource = input<number | null>(null);
  readonly supply = input<Supply | null>(null);
  readonly size = input(20);

  protected readonly v = computed((): { name: string; color: string; shapes?: readonly IconShape[]; glyph?: string; path?: string; text?: string } => {
    const supply = this.supply();
    if (supply) {
      const s = SUPPLY_ICONS[supply];
      return { name: s.name, color: s.color, path: s.d };
    }
    const r = RESOURCES[this.resource() ?? 0];
    if (r.icon) return { name: r.name, color: r.color, shapes: ICONS[r.icon] };
    if (r.glyph) return { name: r.name, color: r.color, glyph: r.glyph };
    return { name: r.name, color: r.color, text: r.symbol ?? r.name.slice(0, 2) };
  });
}
