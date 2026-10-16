import { Component, computed, inject } from '@angular/core';

import type { FrontSide } from '../../generated/FrontSide';
import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { flagDataUrl } from '../render/flags';
import { full, short } from './format';
import { GoodsIcon } from './goods-icon';

/**
 * Aktywne fronty (prawy dolny róg, z minimalizacją): ataki gracza i ataki na jego prowincje – strony
 * (flaga, wojsko, zaopatrzenie w użyciu), prowincja, % okupacji, „Idź do” – płynny przelot kamery
 * na środek frontu.
 */
@Component({
  selector: 'app-fronts-panel',
  imports: [GoodsIcon],
  templateUrl: './fronts-panel.html',
  styleUrls: ['./ui.css', './fronts-panel.css'],
})
export class FrontsPanel {
  protected readonly store = inject(MapStore);
  private readonly session = inject(GameSession);

  protected readonly view = computed(() => {
    const fronts = this.session.fronts();
    if (fronts.length === 0) return null;
    const nations = this.session.owners()?.nations ?? [];
    const side = (s: FrontSide) => {
      const nation = nations.indexOf(s.player) + 1;
      return {
        name: this.session.playerName(s.player),
        flag: nation > 0 ? flagDataUrl(nation, 18, 12) : '',
        troops: short(s.troops),
        troopsTitle: `${full(s.troops)} wojska`,
        supply: s.supply,
        tiles: s.tiles,
      };
    };
    const provinces = this.store.map()?.provinces;
    const name = (p: number) => provinces?.[p - 1]?.name || `#${p}`;
    return fronts.map((f) => ({
      key: `${f.provinces.join(',')}:${f.attacker.player}`,
      province: f.provinces[0] ?? 0,
      name: f.provinces.map(name).join(', '),
      landing: f.landing,
      natives: f.natives,
      lost: short(f.lost),
      mine: f.mine,
      attacker: side(f.attacker),
      defenders: f.defenders.map(side),
      neutral: f.neutralTiles,
      occupation: Math.round(f.occupation * 100),
      x: f.centerX,
      y: f.centerY,
      area: f.provinces.reduce((s, p) => s + (provinces?.[p - 1]?.area ?? 0), 0) || 400,
    }));
  });

  protected goTo(x: number, y: number, area: number): void {
    this.store.flyTo(Math.round(x), Math.round(y), area);
  }

  protected toggle(): void {
    this.store.frontsPanel.update((s) => (s === 'open' ? 'min' : 'open'));
  }
}
