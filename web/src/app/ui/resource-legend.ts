import { Component, computed, inject } from '@angular/core';

import { MapStore } from '../game/map-store';
import { RESOURCE_GROUPS, RESOURCES } from '../render/resources';

/** Na wąskim ekranie legenda startuje zwinięta, żeby nie zasłaniać mapy. */
const NARROW = '(max-width: 640px)';

/**
 * Legenda mapy zasobów (prawy dolny róg, tylko w widoku „Zasoby”): kolory zasobów grupami
 * i liczba prowincji z każdym zasobem na bieżącej mapie.
 */
@Component({
  selector: 'app-resource-legend',
  templateUrl: './resource-legend.html',
  styleUrls: ['./ui.css', './resource-legend.css'],
})
export class ResourceLegend {
  protected readonly store = inject(MapStore);
  protected readonly open = !globalThis.matchMedia?.(NARROW).matches;

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
    return RESOURCE_GROUPS.map((g) => ({
      name: g.name,
      items: g.resources.map((r) => ({ ...RESOURCES[r], count: counts?.[r] ?? null })),
    }));
  });
}
