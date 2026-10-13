import { Component, computed, inject } from '@angular/core';

import { MapStore } from '../game/map-store';
import { ICONS, RESOURCE_GROUPS, RESOURCE_KINDS, RESOURCES } from '../render/resources';

/** Na wąskim ekranie legenda startuje zwinięta, żeby nie zasłaniać mapy. */
const NARROW = '(max-width: 640px)';

/**
 * Legenda mapy zasobów (prawy dolny róg, tylko w widoku „Zasoby”): kolory zasobów grupami, z obwódką
 * rodzaju (podstawowy, strategiczny, luksusowy) albo – energia i metale – z ikoną lub symbolem pierwiastka
 * jak na mapie, i liczba prowincji z każdym zasobem na bieżącej mapie. Jest też filtrem: kliknięcie zasobu włącza/wyłącza go na mapie,
 * kliknięcie kategorii zostawia tylko ją, „Wszystkie”/„Żadne” – wszystkie.
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
  /** Klucz obwódek (rodzaje zasobów). */
  protected readonly kinds = Object.values(RESOURCE_KINDS);

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
        /** Obwódka jak na mapie (według rodzaju); null w grupach ze znakami (energia, metale). */
        rim: g.marks ? null : RESOURCE_KINDS[RESOURCES[r].kind],
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

  /** Zaznacza albo odznacza wszystkie (przyciski są w nagłówku `summary` – bez zwijania legendy). */
  protected setAll(on: boolean, event: Event): void {
    event.preventDefault();
    event.stopPropagation();
    this.store.resourceFilter.set(RESOURCES.map(() => on));
  }
}
