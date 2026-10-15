import { Component, computed, inject } from '@angular/core';

import { MapStore } from '../game/map-store';
import { ICONS, RESOURCE_GROUPS, RESOURCES, SUPPLY_CATEGORIES, categoryResources } from '../render/resources';

/** Na wąskim ekranie legenda startuje zwinięta, żeby nie zasłaniać mapy. */
const NARROW = '(max-width: 640px)';

/** Czy filtr pokazuje dokładnie te zasoby. */
function showsOnly(filter: readonly boolean[], keep: readonly number[]): boolean {
  return filter.every((on, r) => on === keep.includes(r));
}

/**
 * Legenda mapy zasobów (prawy dolny róg, tylko w widoku „Zasoby”): kolory zasobów grupami (energia
 * i metale – z ikoną lub symbolem pierwiastka jak na mapie) i liczba prowincji z każdym zasobem na
 * bieżącej mapie. Jest też filtrem: kliknięcie zasobu włącza/wyłącza go na mapie, kliknięcie grupy
 * albo kategorii zaopatrzenia (surowce potrzebne do lekkiego wojskowego, paliwa…) zostawia tylko je –
 * ponowne kliknięcie przywraca wszystkie; „Wszystkie” [W] / „Żadne” [Z].
 */
@Component({
  selector: 'app-resource-legend',
  templateUrl: './resource-legend.html',
  styleUrls: ['./ui.css', './resource-legend.css'],
})
export class ResourceLegend {
  protected readonly store = inject(MapStore);
  protected readonly open = !globalThis.matchMedia?.(NARROW).matches;
  protected readonly icons = ICONS;
  /**
   * Suma poziomów uniwersytetów gracza – syntetyki dochodzą do kategorii po progach. Uniwersytetów
   * jeszcze nie ma (etap 7 w docs/gdd/plan.md), więc na razie 0.
   */
  private readonly universities = computed(() => 0);

  /** Kategorie zaopatrzenia – przyciski filtra (aktywna = filtr pokazuje dokładnie jej surowce). */
  protected readonly categories = computed(() => {
    const filter = this.store.resourceFilter();
    return SUPPLY_CATEGORIES.map((c, index) => {
      const resources = categoryResources(c, this.universities());
      return {
        index,
        name: c.name,
        resources,
        active: showsOnly(filter, resources),
        title: `Pokaż tylko surowce: ${c.name} – ${resources.map((r) => RESOURCES[r].name).join(', ')}`,
      };
    });
  });

  /** Liczba prowincji z każdym zasobem; null, dopóki faza 3 nie przydzieli zasobów. */
  private readonly counts = computed(() => {
    const map = this.store.map();
    if (!map?.polished) return null;
    const counts = new Array<number>(RESOURCES.length).fill(0);
    for (const p of map.provinces) counts[p.resource]++;
    return counts;
  });

  protected readonly groups = computed(() => {
    const counts = this.counts();
    const filter = this.store.resourceFilter();
    return RESOURCE_GROUPS.map((g, index) => ({
      index,
      name: g.name,
      active: showsOnly(filter, g.resources),
      items: g.resources.map((r) => ({
        ...RESOURCES[r],
        index: r,
        on: filter[r],
        count: counts?.[r] ?? null,
      })),
    }));
  });

  protected toggle(resource: number): void {
    this.store.resourceFilter.update((f) => f.map((on, r) => (r === resource ? !on : on)));
  }

  /** Zostawia na mapie tylko zasoby z tej grupy; ponownie – wszystkie. */
  protected only(group: number): void {
    this.keepOnly(RESOURCE_GROUPS[group].resources);
  }

  /** Zostawia na mapie tylko surowce danej kategorii zaopatrzenia; ponownie – wszystkie. */
  protected onlyCategory(category: number): void {
    this.keepOnly(this.categories()[category].resources);
  }

  /** Zaznacza albo odznacza wszystkie (przyciski są w nagłówku `summary` – bez zwijania legendy). */
  protected setAll(on: boolean, event: Event): void {
    event.preventDefault();
    event.stopPropagation();
    this.store.resourceFilter.set(RESOURCES.map(() => on));
  }

  private keepOnly(keep: readonly number[]): void {
    const filter = this.store.resourceFilter();
    this.store.resourceFilter.set(RESOURCES.map((_, r) => (showsOnly(filter, keep) ? true : keep.includes(r))));
  }
}
