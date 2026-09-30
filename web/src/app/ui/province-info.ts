import { Component, computed, inject } from '@angular/core';

import { MapStore } from '../game/map-store';
import { BIOMES, Terrain } from '../render/terrain';

/** Pasek odchyłu wartości sięga ±tyle (0.5 = ±50% średniej); dalej jest przycięty. */
const DEVIATION_RANGE = 0.5;

/** Ramka w lewym dolnym rogu: prowincja pod kursorem, a gdy kursor jest poza lądem – zaznaczona. */
@Component({
  selector: 'app-province-info',
  templateUrl: './province-info.html',
  styleUrl: './ui.css',
})
export class ProvinceInfo {
  private readonly store = inject(MapStore);

  /** Ukształtowanie i biomy wszystkich prowincji – jedno przejście po kaflach na mapę. */
  private readonly tally = computed(() => {
    const map = this.store.map();
    if (!map) return null;
    const count = map.provinces.length + 1;
    const relief = new Uint32Array(count * 3); // niziny (z rzekami), wyżyny, góry
    const biomes = new Uint32Array(count * BIOMES.length);
    const { province: ids, terrain, biome } = map;
    for (let i = 0; i < ids.length; i++) {
      const id = ids[i];
      if (id === 0) continue;
      const t = terrain[i];
      const k = t === Terrain.Highlands ? 1 : t === Terrain.Mountains ? 2 : 0;
      relief[id * 3 + k]++;
      biomes[id * BIOMES.length + biome[i]]++;
    }
    return { relief, biomes };
  });

  protected readonly info = computed(() => {
    const map = this.store.map();
    const tally = this.tally();
    const hovered = this.store.hoveredProvince();
    const selected = this.store.selectedProvince();
    const id = hovered || selected;
    const province = map && id > 0 ? map.provinces[id - 1] : undefined;
    if (!map || !province || !tally) return null;
    const [plains, highlands, mountains] = tally.relief.subarray(id * 3, id * 3 + 3);
    const total = Math.max(1, plains + highlands + mountains);
    const pct = (n: number) => Math.round((n / total) * 100);
    const own = Array.from(tally.biomes.subarray(id * BIOMES.length, (id + 1) * BIOMES.length));
    const top = own.indexOf(Math.max(...own));
    // Odchył od ustalonej średniej wartości prowincji (parametr generatora).
    const target = map.params.provinceValue;
    const deviation = target > 0 ? province.value / target - 1 : 0;
    const k = Math.max(-1, Math.min(1, deviation / DEVIATION_RANGE)) * 50; // % szerokości paska od środka
    const size = Math.abs(deviation);
    return {
      target,
      deviation: Math.round(deviation * 100),
      bar: { left: 50 + Math.min(0, k), width: Math.abs(k) },
      level: size <= 0.1 ? 'ok' : size <= 0.25 ? 'warn' : 'bad',
      province,
      selected: id === selected,
      plains: pct(plains),
      highlands: pct(highlands),
      mountains: pct(mountains),
      biome: BIOMES[top]?.name ?? '–',
      fertility: Math.round((province.fertility / 255) * 100),
    };
  });
}
