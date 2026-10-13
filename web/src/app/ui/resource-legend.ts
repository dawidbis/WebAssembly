import { Component, computed, inject } from '@angular/core';

import { MapStore } from '../game/map-store';
import { ICONS, RESOURCE_GROUPS, RESOURCE_KINDS, RESOURCES, type ResourceKind } from '../render/resources';

/** Na wąskim ekranie legenda startuje zwinięta, żeby nie zasłaniać mapy. */
const NARROW = '(max-width: 640px)';

/**
 * Legenda mapy zasobów (prawy dolny róg, tylko w widoku „Zasoby”): kolory zasobów grupami (energia
 * i metale – z ikoną lub symbolem pierwiastka jak na mapie) i liczba prowincji z każdym zasobem na
 * bieżącej mapie. Jest też filtrem: kliknięcie zasobu włącza/wyłącza go na mapie, kliknięcie kategorii
 * albo rodzaju (podstawowe, strategiczne, luksusowe) zostawia tylko je, „Wszystkie”/„Żadne” – wszystkie.
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
  /** Rodzaje zasobów – przyciski filtra. */
  protected readonly kinds = Object.entries(RESOURCE_KINDS).map(([kind, name]) => ({ kind: kind as ResourceKind, name }));

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

  /** Zostawia na mapie tylko zasoby z tej kategorii. */
  protected only(group: number): void {
    const keep = RESOURCE_GROUPS[group].resources;
    this.store.resourceFilter.set(RESOURCES.map((_, r) => keep.includes(r)));
  }

  /** Zostawia na mapie tylko zasoby danego rodzaju (podstawowe, strategiczne, luksusowe). */
  protected onlyKind(kind: ResourceKind): void {
    this.store.resourceFilter.set(RESOURCES.map((r) => r.kind === kind));
  }

  /** Zaznacza albo odznacza wszystkie (przyciski są w nagłówku `summary` – bez zwijania legendy). */
  protected setAll(on: boolean, event: Event): void {
    event.preventDefault();
    event.stopPropagation();
    this.store.resourceFilter.set(RESOURCES.map(() => on));
  }
}
