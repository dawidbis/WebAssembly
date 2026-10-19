import { Component, computed, inject } from '@angular/core';

import type { Supply } from '../../generated/Supply';
import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { amount } from './format';
import { GoodsIcon, SUPPLY_ICONS } from './goods-icon';

/** Rodzaje zaopatrzenia w kolejności `Supply as usize` (jak w oknie zapasów). */
const SUPPLIES = Object.keys(SUPPLY_ICONS) as Supply[];

/**
 * Pasek zaopatrzenia pod paskiem zasobów (PC i telefon): każdy rodzaj – mała ikona, zapas i bilans na dzień
 * (zielony / czerwony). Bez surowców – te są w oknie zapasów, które otwiera kliknięcie paska.
 */
@Component({
  selector: 'app-supply-strip',
  imports: [GoodsIcon],
  template: `
    @if (items(); as list) {
      <button
        type="button"
        class="strip"
        [class.open]="store.stockWindow() !== 'closed'"
        (click)="toggleStock()"
        aria-controls="stock-window"
        [attr.aria-expanded]="store.stockWindow() !== 'closed'"
        title="Zaopatrzenie: zapas i bilans na dzień – kliknij, aby otworzyć okno zapasów"
      >
        @for (s of list; track s.supply) {
          <span class="item" [class.empty]="s.empty" [title]="s.title">
            <app-goods-icon [supply]="s.supply" [size]="15" />
            <span class="amount">{{ s.amount }}</span>
            @if (s.rate) {
              <small class="rate" [class.down]="s.down">{{ s.rate }}</small>
            }
          </span>
        }
      </button>
    }
  `,
  styles: `
    :host { display: block; min-width: 0; pointer-events: auto; }
    :host .strip { max-width: 100%; }
    .strip {
      display: flex; flex-wrap: wrap; align-items: center; gap: 2px 10px; padding: 3px 8px; color: #d8e2ea;
      font: 12px/1.3 system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif; font-variant-numeric: tabular-nums;
      background: rgb(12 24 36 / 0.85); border: 1px solid #243847; border-radius: 6px; cursor: pointer; text-align: left;
    }
    .strip:hover, .strip.open { background: #1b2d3c; }
    .strip:focus-visible { outline: 2px solid #6fbf9a; outline-offset: 1px; }
    .item { display: inline-flex; align-items: center; gap: 3px; white-space: nowrap; }
    .item.empty { opacity: 0.45; }
    .amount { font-weight: 600; }
    .rate { color: #8fd6ae; font-size: 10px; }
    .rate.down { color: #ee8f82; }
    @media (max-width: 600px) {
      .strip { gap: 1px 7px; padding: 2px 6px; font-size: 11px; }
      .rate { font-size: 9px; }
    }
  `,
})
export class SupplyStrip {
  private readonly session = inject(GameSession);
  protected readonly store = inject(MapStore);

  protected readonly items = computed(() => {
    const e = this.session.myEconomy();
    if (!e) return null;
    return SUPPLIES.map((supply, i) => {
      const stock = e.supplies[i] ?? 0;
      const perDay = (e.supplyProduction[i] ?? 0) - (e.supplyUse[i] ?? 0);
      const rate = Math.abs(perDay) < 0.05 ? '' : `${perDay > 0 ? '+' : '−'}${amount(Math.abs(perDay))}`;
      return {
        supply,
        amount: amount(stock),
        rate,
        down: perDay < 0,
        empty: stock === 0 && !rate,
        title: `${SUPPLY_ICONS[supply].name}: ${amount(stock)}${rate ? `, ${rate} na dzień` : ''}`,
      };
    });
  });

  protected toggleStock(): void {
    this.store.stockWindow.update((w) => (w === 'closed' ? 'open' : 'closed'));
  }
}
