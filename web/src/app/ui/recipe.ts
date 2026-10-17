import { Component, computed, input, output } from '@angular/core';

import type { IngredientReport } from '../../generated/IngredientReport';
import type { Supply } from '../../generated/Supply';
import { amount } from './format';
import { GoodsIcon } from './goods-icon';

/** Stan puli państwa do receptury: zapasy i zmiana na dzień (produkcja − zużycie). */
export interface PoolState {
  stock: readonly number[];
  supplies: readonly number[];
  resourceRate: readonly number[];
  supplyRate: readonly number[];
}

/** „Na ile starczy”: dni przy ujemnym bilansie, inaczej pusto. */
function lasts(stock: number, rate: number): string {
  if (rate >= -1e-9) return '';
  const days = stock / -rate;
  return days >= 999 ? '' : days < 1 ? '< 1 d' : `${Math.floor(days)} d`;
}

/**
 * Receptura jako „funkcja”: składnik (ikony zamienników z ilością na sztukę) + … → produkt. Z pulą
 * państwa – pod ikonami zapas, przy wejściu w użyciu zaznaczenie i czas do wyczerpania, przy produkcie
 * zapas i na ile starczy przy obecnym zużyciu.
 */
@Component({
  selector: 'app-recipe',
  imports: [GoodsIcon],
  templateUrl: './recipe.html',
  styleUrl: './recipe.css',
})
export class Recipe {
  readonly ingredients = input.required<IngredientReport[]>();
  readonly product = input.required<Supply>();
  readonly pool = input<PoolState | null>(null);
  /** Numer składnika, którego brakuje (wyróżniony). */
  readonly short = input<number | null>(null);
  /** Poziomy uniwersytetów państwa (wejścia wymagające więcej – wyszarzone). */
  readonly universities = input(Number.MAX_SAFE_INTEGER);
  readonly size = input(20);
  /** Wybór wejść klikiem (fabryka gracza): `choose` – składnik i zamiennik albo `null` (automatycznie). */
  readonly interactive = input(false);
  readonly choose = output<{ ingredient: number; option: number | null }>();

  protected pick(ingredient: number, option: number, used: boolean, manual: boolean, locked: boolean): void {
    if (!this.interactive() || locked) return;
    this.choose.emit({ ingredient, option: used && manual ? null : option });
  }

  /** Enter albo spacja na zamienniku – jak kliknięcie. */
  protected onKey(event: KeyboardEvent, ingredient: number, option: number, used: boolean, manual: boolean, locked: boolean): void {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    this.pick(ingredient, option, used, manual, locked);
  }

  protected readonly view = computed(() => {
    const pool = this.pool();
    const ingredients = this.ingredients().map((g, k) => ({
      short: this.short() === k,
      options: g.options.map((o, i) => {
        const stock = pool ? (o.resource !== null ? pool.stock[o.resource] : pool.supplies[supplyIndex(o.supply!)]) ?? 0 : null;
        const rate = pool ? (o.resource !== null ? pool.resourceRate[o.resource] : pool.supplyRate[supplyIndex(o.supply!)]) ?? 0 : 0;
        return {
          resource: o.resource,
          supply: o.supply,
          per: amount(o.amount).replace(/,0$/, ''),
          stock: stock === null ? '' : amount(stock),
          lasts: g.used === i && stock !== null ? lasts(stock, rate) : '',
          used: g.used === i,
          manual: g.manual && g.used === i,
          locked: o.universities > this.universities(),
          title:
            (o.universities > 0 ? `Od ${o.universities} poziomów uniwersytetów. ` : '') +
            (!this.interactive()
              ? ''
              : g.used === i
                ? g.manual
                  ? 'Wybrane ręcznie – kliknij, aby fabryka znów wybierała sama'
                  : 'Wybrane automatycznie (najwięcej sztuk, do wyczerpania) – kliknij, aby ustalić na stałe'
                : 'Kliknij, aby fabryka brała ten zamiennik'),
        };
      }),
    }));
    const p = supplyIndex(this.product());
    return {
      ingredients,
      product: pool
        ? { stock: amount(pool.supplies[p] ?? 0), lasts: lasts(pool.supplies[p] ?? 0, pool.supplyRate[p] ?? 0) }
        : null,
    };
  });
}

const SUPPLY_ORDER: readonly Supply[] = [
  'lightMilitary',
  'standardMilitary',
  'heavyMilitary',
  'fuel',
  'basicCivil',
  'luxuryCivil',
  'engineeringMaterials',
];

/** Indeks rodzaju zaopatrzenia w tablicach raportu (kolejność `goods::Supply`). */
export function supplyIndex(s: Supply): number {
  return SUPPLY_ORDER.indexOf(s);
}
