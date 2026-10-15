import { Component, computed, inject, input } from '@angular/core';

import type { Supply } from '../../generated/Supply';
import { GameSession } from '../game/game-session';
import { amount } from './format';
import { GoodsIcon } from './goods-icon';
import { Recipe } from './recipe';

/**
 * Panel produkcji fabryki prowincji (okno budynków, okienko budynku z mapy): produkt (ikony rodzajów),
 * receptura z zapasami – zamienniki do wyboru klikiem (ręcznie albo automatycznie), produkcja na dzień,
 * przezbrojenie. Dane – raport prowincji z workera (`GameSession.provinceReport`).
 */
@Component({
  selector: 'app-factory-panel',
  imports: [GoodsIcon, Recipe],
  template: `
    @if (view(); as f) {
      <div class="factory" [class.short]="f.short && f.working && !f.retool">
        <div class="line">
          <span class="label">Produkcja</span>
          <span class="products" role="radiogroup" aria-label="Produkcja fabryki">
            @for (p of f.products; track p.supply) {
              <button
                type="button"
                class="product"
                role="radio"
                [attr.aria-checked]="p.supply === f.product"
                [class.on]="p.supply === f.product"
                [disabled]="p.locked || !f.mine"
                [title]="p.title"
                (click)="setProduct(p.supply)"
              >
                <app-goods-icon [supply]="p.supply" [size]="18" />
              </button>
            }
          </span>
        </div>
        <app-recipe
          [ingredients]="f.ingredients"
          [product]="f.product"
          [pool]="f.mine ? session.myPool() : null"
          [short]="f.shortIngredient"
          [universities]="f.universities"
          [interactive]="f.mine"
          (choose)="choose($event)"
        />
        @if (f.working) {
          <small class="status">{{ f.retool ? 'Przezbrojenie: ' + f.retool + ' d' : f.output }}</small>
        }
      </div>
    }
  `,
  styles: `
    .factory { display: grid; gap: 6px; padding: 8px; background: #142230; border-radius: 6px; }
    .line { display: flex; align-items: center; gap: 8px; }
    .label { flex: 1; color: #8ea3b4; font-size: 12px; }
    .products { display: inline-flex; gap: 2px; }
    .product { padding: 1px; line-height: 0; background: none; border: 1px solid transparent; border-radius: 5px; cursor: pointer; }
    .product.on { border-color: #6fbf9a; background: rgb(111 191 154 / 0.15); }
    .product:disabled { opacity: 0.3; cursor: not-allowed; }
    .product.on:disabled { opacity: 1; }
    .status { color: #8fd6ae; font-size: 12px; }
    .factory.short .status { color: #e8cc72; }
  `,
})
export class FactoryPanel {
  protected readonly session = inject(GameSession);
  readonly province = input.required<number>();

  protected readonly view = computed(() => {
    const r = this.session.provinceReport();
    if (!r || r.province !== this.province()) return null;
    const f = r.factory;
    const level = r.options.find((o) => o.building === 'factory')?.level ?? 0;
    if (level === 0 && r.construction?.building !== 'factory') return null;
    return {
      mine: r.mine,
      product: f.product,
      ingredients: f.ingredients,
      shortIngredient: f.short,
      products: f.products.map((p) => ({
        supply: p.supply,
        locked: p.commandLevel !== null,
        title: p.commandLevel !== null ? `${p.name} – od centrum dowodzenia ${p.commandLevel}` : `${p.name}: ${p.recipe}`,
      })),
      working: level > 0,
      output: `${amount(f.outputPerDay)} / ${amount(f.capacityPerDay)} na dzień`,
      short: f.outputPerDay < f.capacityPerDay - 1e-9,
      retool: f.retoolDays > 0 ? Math.ceil(f.retoolDays) : 0,
      universities: this.session.myEconomy()?.universities ?? 0,
    };
  });

  protected setProduct(supply: Supply): void {
    this.session.setFactory(this.province(), supply);
  }

  protected choose(e: { ingredient: number; option: number | null }): void {
    this.session.setFactoryInput(this.province(), e.ingredient, e.option);
  }
}
