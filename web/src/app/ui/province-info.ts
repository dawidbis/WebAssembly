import { Component, computed, inject } from '@angular/core';

import { MapStore } from '../game/map-store';
import { BIOMES, Terrain } from '../render/terrain';

/** Pasek odchyłu wielkości sięga ±tyle (0.5 = ±50% średniej); dalej jest przycięty. */
const DEVIATION_RANGE = 0.5;

/**
 * Ramka w lewym dolnym rogu: prowincja pod kursorem, a gdy kursor jest poza lądem – zaznaczona.
 * Nad górami: informacja, że są nieprzechodnie i niczyje.
 */
@Component({
  selector: 'app-province-info',
  templateUrl: './province-info.html',
  styleUrl: './ui.css',
})
export class ProvinceInfo {
  protected readonly store = inject(MapStore);

  /** Ukształtowanie wszystkich prowincji – jedno przejście po kaflach na mapę. */
  private readonly tally = computed(() => {
    const map = this.store.map();
    if (!map) return null;
    const relief = new Uint32Array((map.provinces.length + 1) * 2); // niziny, wyżyny – góry nie należą do prowincji
    const { province: ids, terrain } = map;
    for (let i = 0; i < ids.length; i++) {
      const id = ids[i];
      if (id !== 0) relief[id * 2 + (terrain[i] === Terrain.Highlands ? 1 : 0)]++;
    }
    return relief;
  });

  protected readonly info = computed(() => {
    const map = this.store.map();
    const tally = this.tally();
    const hovered = this.store.hoveredProvince();
    const selected = this.store.selectedProvince();
    const id = hovered || selected;
    const province = map && id > 0 ? map.provinces[id - 1] : undefined;
    if (!map || !province || !tally) return null;
    const [plains, highlands] = tally.subarray(id * 2, id * 2 + 2);
    const total = Math.max(1, plains + highlands);
    const pct = (n: number) => Math.round((n / total) * 100);
    // Odchył od średniej liczby kafli prowincji na tej mapie.
    const mean = map.stats.provinceAreaMean;
    const deviation = mean > 0 ? province.area / mean - 1 : 0;
    const k = Math.max(-1, Math.min(1, deviation / DEVIATION_RANGE)) * 50; // % szerokości paska od środka
    const size = Math.abs(deviation);
    return {
      mean: Math.round(mean),
      deviation: Math.round(deviation * 100),
      bar: { left: 50 + Math.min(0, k), width: Math.abs(k) },
      level: size <= 0.1 ? 'ok' : size <= 0.25 ? 'warn' : 'bad',
      province,
      selected: id === selected,
      plains: pct(plains),
      highlands: pct(highlands),
      biome: BIOMES[province.biome]?.name ?? '–',
      yesNo: (v: boolean) => (v ? 'tak' : 'nie'),
    };
  });
}
