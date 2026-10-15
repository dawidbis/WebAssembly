import { Component, computed, inject } from '@angular/core';

import { GameSession } from '../game/game-session';
import { MapStore, type Wasteland } from '../game/map-store';
import { flagDataUrl } from '../render/flags';
import { RESOURCES } from '../render/resources';
import { BIOMES, Terrain } from '../render/terrain';
import { ProvinceBuildings } from './province-buildings';

/** Pasek odchyłu wielkości sięga ±tyle (0.5 = ±50% średniej); dalej jest przycięty. */
const DEVIATION_RANGE = 0.5;

/** Kafel leśny w rozgrywce: `forest` ≥ 128 (README „Roślinność”). */
const FOREST_TILE = 128;

/** Zalesienie i wyżyny od tylu procent są wyróżnione kolorem. */
const HIGHLIGHT_SHARE = 50;

/** Teren, z którym liczymy styk kafli prowincji – w kolejności liczników od `Tally.Sea`. */
const BORDER_TERRAIN: readonly number[] = [Terrain.Ocean, Terrain.River, Terrain.Lake, Terrain.Mountains];

/** Indeksy liczników jednej prowincji w `tally`. */
const Tally = { Plains: 0, Highlands: 1, Forest: 2, Sea: 3, River: 4, Lake: 5, Mountains: 6 } as const;
const TALLY = 7;

/** Kolor odchyłu: do ±10% zielony, do ±25% żółty, dalej czerwony. */
function deviationLevel(size: number): 'ok' | 'warn' | 'bad' {
  if (size <= 0.1) return 'ok';
  return size <= 0.25 ? 'warn' : 'bad';
}

/**
 * Ramka w lewym dolnym rogu: zaznaczona prowincja (kliknięcie), a bez zaznaczenia – prowincja pod kursorem.
 * Nad górami: informacja, że są nieprzechodnie i niczyje. W grze – budynki prowincji (`province-buildings`).
 */
@Component({
  selector: 'app-province-info',
  templateUrl: './province-info.html',
  styleUrls: ['./ui.css', './province-info.css'],
  imports: [ProvinceBuildings],
})
export class ProvinceInfo {
  protected readonly store = inject(MapStore);
  private readonly session = inject(GameSession);
  protected readonly wasteTitle: Record<Wasteland, string> = { mountains: 'Góry', glacier: 'Lodowiec', enclave: 'Enklawa' };
  protected readonly wasteText: Record<Wasteland, string> = {
    mountains: 'Nieprzechodnie i niczyje – nie należą do żadnej prowincji.',
    glacier: 'Nieprzechodni i niczyj – nie należy do żadnej prowincji.',
    enclave: 'Dolina odcięta od świata dalej, niż sięga najdłuższy tunel – niczyja. Mieszka tu yeti.',
  };

  /**
   * Liczniki wszystkich prowincji – jedno przejście po kaflach na mapę: niziny, wyżyny (góry nie należą
   * do prowincji), kafle leśne i kafle prowincji stykające się bokiem z morzem, rzeką, jeziorem, górami
   * (ta sama reguła co flagi `coastal`/`river`/`lake`/`mountains` w `provinces.rs`, więc licznik > 0 ⇔ flaga).
   */
  private readonly tally = computed(() => {
    const map = this.store.map();
    if (!map) return null;
    const counts = new Uint32Array((map.provinces.length + 1) * TALLY);
    const { province: ids, terrain, forest, width: w, height: h } = map;
    const touches = new Uint8Array(BORDER_TERRAIN.length);
    for (let i = 0; i < ids.length; i++) {
      const id = ids[i];
      if (id === 0) continue;
      const o = id * TALLY;
      counts[o + (terrain[i] === Terrain.Highlands ? Tally.Highlands : Tally.Plains)]++;
      if (forest[i] >= FOREST_TILE) counts[o + Tally.Forest]++;
      const x = i % w;
      const y = (i - x) / w;
      touches.fill(0);
      const look = (j: number) => {
        const k = BORDER_TERRAIN.indexOf(terrain[j]);
        if (k >= 0) touches[k] = 1;
      };
      if (x > 0) look(i - 1);
      if (x < w - 1) look(i + 1);
      if (y > 0) look(i - w);
      if (y < h - 1) look(i + w);
      for (let k = 0; k < touches.length; k++) counts[o + Tally.Sea + k] += touches[k];
    }
    return counts;
  });

  protected readonly info = computed(() => {
    const map = this.store.map();
    const tally = this.tally();
    const hovered = this.store.hoveredProvince();
    const selected = this.store.selectedProvince();
    // Zaznaczona prowincja ma pierwszeństwo – najechanie na inną nie podmienia ramki.
    const id = selected || hovered;
    const province = map && id > 0 ? map.provinces[id - 1] : undefined;
    if (!map || !province || !tally) return null;
    const c = tally.subarray(id * TALLY, (id + 1) * TALLY);
    const total = Math.max(1, c[Tally.Plains] + c[Tally.Highlands]);
    const pct = (n: number) => Math.round((n / total) * 100);
    const highlands = pct(c[Tally.Highlands]);
    const forest = pct(c[Tally.Forest]);
    // Odchył od średniej liczby kafli prowincji na tej mapie.
    const mean = map.stats.provinceAreaMean;
    const deviation = mean > 0 ? province.area / mean - 1 : 0;
    const k = Math.max(-1, Math.min(1, deviation / DEVIATION_RANGE)) * 50; // % szerokości paska od środka
    const size = Math.abs(deviation);
    return {
      mean: Math.round(mean),
      deviation: Math.round(deviation * 100),
      bar: { left: 50 + Math.min(0, k), width: Math.abs(k) },
      level: deviationLevel(size),
      province,
      selected: id === selected,
      plains: 100 - highlands,
      highlands,
      upland: highlands >= HIGHLIGHT_SHARE,
      forest,
      wooded: forest >= HIGHLIGHT_SHARE,
      biome: BIOMES[province.biome]?.name ?? '–',
      resource: RESOURCES[province.resource],
      owners: this.ownersOf(id, province.area),
      // Liczba kafli prowincji stykających się bokiem z danym terenem (0 = nie graniczy).
      borders: [
        { label: 'Graniczy z morzem', tiles: c[Tally.Sea] },
        { label: 'Graniczy z rzeką', tiles: c[Tally.River] },
        { label: 'Graniczy z jeziorem', tiles: c[Tally.Lake] },
        { label: 'Graniczy z górami', tiles: c[Tally.Mountains] },
      ],
    };
  });

  /**
   * Właściciele prowincji w bieżącej grze (GDD: prowincja może być współdzielona): gracz, kolor państwa
   * i udział w kaflach. `null` – bez gry albo na lokalnym podglądzie (wiersza nie ma), `[]` – niczyja.
   */
  private ownersOf(id: number, area: number): { name: string; flag: string; share: number }[] | null {
    const owners = this.session.owners();
    if (!owners || !this.session.showsGameMap()) return null;
    return (owners.shares[id - 1] ?? [])
      .map(([nation, tiles]) => ({
        name: this.session.playerName(owners.nations[nation - 1]),
        flag: flagDataUrl(nation),
        share: Math.round((tiles / area) * 100),
      }))
      .sort((a, b) => b.share - a.share); // największy udział pierwszy – jego flaga jest w nagłówku
  }
}
