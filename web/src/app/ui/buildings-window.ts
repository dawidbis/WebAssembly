import { Component, computed, inject } from '@angular/core';

import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { FloatingWindow } from './floating-window';
import { ProvinceBuildings } from './province-buildings';
import { ProvinceSummary } from './province-summary';

/**
 * Okno prowincji: otwiera się po zaznaczeniu prowincji w grze (kliknięcie albo menu kołowe), gdy włączona jest
 * opcja „okno prowincji” (ikonka pod flagą; X w oknie ją wyłącza). Przeciągane albo zadokowane w lewym dolnym rogu. Na górze najważniejsze dane prowincji
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
        [docked]="store.provinceDocked()"
        (closed)="store.setProvinceWindow(false)"
      >
        @if (v.capital) {
          <span title-extra class="capital" title="Stolica – centrum dowodzenia">★</span>
        }
        @if (v.cutoff) {
          <svg
            title-extra
            class="cutoff"
            viewBox="0 0 24 24"
            role="img"
            aria-label="Przyczółek odcięty od kraju"
          >
            <title>Przyczółek odcięty od kraju: posiłki tylko desantem, ataki stąd – z garnizonu. Połączysz go lądem z krajem albo zdobyciem portu.</title>
            <path d="M12 3a2 2 0 1 0 0 4 2 2 0 1 0 0-4M12 7v13M8.5 10.5h7M5 13.5a7 6.5 0 0 0 14 0M3.5 3.5l17 17" />
          </svg>
        }
        <button
          title-extra
          type="button"
          class="win-btn dock"
          [class.on]="store.provinceDocked()"
          (click)="store.setProvinceDocked(!store.provinceDocked())"
          [title]="store.provinceDocked() ? 'Odepnij okno (przeciągane)' : 'Zadokuj w lewym dolnym rogu ekranu'"
          [attr.aria-label]="store.provinceDocked() ? 'Odepnij okno' : 'Zadokuj okno w lewym dolnym rogu'"
          [attr.aria-pressed]="store.provinceDocked()"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 4h16v16H4zM4 14h8v6" /></svg>
        </button>
        <app-province-summary [province]="v.province" />
        <app-province-buildings [province]="v.province" />
      </app-floating-window>
    }
  `,
  styles: `
    .capital { color: #f2d47a; }
    .dock { display: grid; place-items: center; width: 24px; height: 24px; padding: 0; }
    .dock svg { width: 15px; height: 15px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linejoin: round; }
    .dock.on { color: #8fd6ae; border-color: #3f8a6b; }
    .cutoff { width: 18px; height: 18px; fill: none; stroke: #ee8f82; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
  `,
})
export class BuildingsWindow {
  protected readonly store = inject(MapStore);
  private readonly session = inject(GameSession);
  protected readonly initial = { x: Math.max(8, innerWidth - 360), y: 72 };

  protected readonly view = computed(() => {
    const id = this.store.selectedProvince();
    const r = this.session.provinceReport();
    if (!id || !r || r.province !== id || this.session.phase() !== 'playing' || !this.store.provinceWindow()) return null;
    const province = this.store.map()?.provinces[id - 1];
    const capital = (r.options.find((o) => o.building === 'commandCenter')?.level ?? 0) > 0;
    return { province: id, name: province?.name || `#${id}`, capital, cutoff: r.cutoff };
  });
}
