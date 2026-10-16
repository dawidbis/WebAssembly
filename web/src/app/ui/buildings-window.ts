import { Component, computed, inject } from '@angular/core';

import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { FloatingWindow } from './floating-window';
import { ProvinceBuildings } from './province-buildings';
import { ProvinceSummary } from './province-summary';

/**
 * Okno prowincji: otwiera się po zaznaczeniu prowincji w grze (kliknięcie albo menu kołowe), przeciągane,
 * z zamykaniem (zamknięte – do zaznaczenia innej prowincji). Na górze najważniejsze dane prowincji
 * (`province-summary`), niżej budynki – w cudzej i niczyjej prowincji tylko poziomy, bez budowy.
 */
@Component({
  selector: 'app-buildings-window',
  imports: [FloatingWindow, ProvinceBuildings, ProvinceSummary],
  template: `
    @if (view(); as v) {
      <app-floating-window
        [title]="v.name"
        key="mapa.buildingsWindow.pos"
        [width]="340"
        [initial]="initial"
        (closed)="store.buildingsClosedFor.set(v.province)"
      >
        @if (v.capital) {
          <span title-extra class="capital" title="Stolica – centrum dowodzenia">★</span>
        }
        <app-province-summary [province]="v.province" />
        <app-province-buildings [province]="v.province" />
      </app-floating-window>
    }
  `,
  styles: `.capital { color: #f2d47a; }`,
})
export class BuildingsWindow {
  protected readonly store = inject(MapStore);
  private readonly session = inject(GameSession);
  protected readonly initial = { x: Math.max(8, innerWidth - 360), y: 72 };

  protected readonly view = computed(() => {
    const id = this.store.selectedProvince();
    const r = this.session.provinceReport();
    if (!id || !r || r.province !== id || this.session.phase() !== 'playing' || this.store.buildingsClosedFor() === id)
      return null;
    const province = this.store.map()?.provinces[id - 1];
    const capital = (r.options.find((o) => o.building === 'commandCenter')?.level ?? 0) > 0;
    return { province: id, name: province?.name || `#${id}`, capital };
  });
}
