import { Component, computed, inject } from '@angular/core';

import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { FloatingWindow } from './floating-window';
import { ProvinceBuildings } from './province-buildings';

/**
 * Okno budynków: otwiera się po zaznaczeniu własnej prowincji (kliknięcie albo menu kołowe), przeciągane,
 * z zamykaniem (zamknięte – do zaznaczenia innej prowincji).
 */
@Component({
  selector: 'app-buildings-window',
  imports: [FloatingWindow, ProvinceBuildings],
  template: `
    @if (view(); as v) {
      <app-floating-window
        [title]="'Budynki – ' + v.name"
        key="mapa.buildingsWindow.pos"
        [width]="340"
        [initial]="initial"
        (closed)="store.buildingsClosedFor.set(v.province)"
      >
        @if (v.capital) {
          <span title-extra class="capital" title="Stolica – centrum dowodzenia">★</span>
        }
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
    if (!id || !r || r.province !== id || !r.mine || this.store.buildingsClosedFor() === id) return null;
    const province = this.store.map()?.provinces[id - 1];
    const capital = (r.options.find((o) => o.building === 'commandCenter')?.level ?? 0) > 0;
    return { province: id, name: province?.name || `#${id}`, capital };
  });
}
