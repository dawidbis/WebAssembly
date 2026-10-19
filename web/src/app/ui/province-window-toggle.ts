import { Component, inject, input, output } from '@angular/core';

import { MapStore } from '../game/map-store';

/**
 * Ikonka opcji „okno prowincji”: włączona (podświetlona) – kliknięcie prowincji otwiera jej okno; X w oknie
 * prowincji ją wyłącza. Pod flagą w pasku zasobów, a na telefonie także w dymku prowincji – tam z `province`
 * włącza opcję i od razu otwiera okno tej prowincji.
 */
@Component({
  selector: 'app-province-window-toggle',
  template: `
    <button
      type="button"
      class="toggle"
      [class.on]="store.provinceWindow()"
      [attr.aria-pressed]="store.provinceWindow()"
      [title]="title()"
      [attr.aria-label]="title()"
      (click)="toggle()"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5h16v14H4zM4 9h16M8 13h5M8 16h8" /></svg>
    </button>
  `,
  styles: `
    :host { display: inline-flex; pointer-events: auto; }
    .toggle {
      display: grid; place-items: center; width: 28px; height: 26px; padding: 0; color: #8ea3b4;
      background: rgb(12 24 36 / 0.9); border: 1px solid #243847; border-radius: 6px; cursor: pointer;
    }
    .toggle:hover { background: #24394b; }
    .toggle.on { color: #fff; background: #2f6f55; border-color: #3f8a6b; }
    .toggle:focus-visible { outline: 2px solid #6fbf9a; outline-offset: 1px; }
    svg { width: 17px; height: 17px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
  `,
})
export class ProvinceWindowToggle {
  protected readonly store = inject(MapStore);
  /** Prowincja do otwarcia od razu (dymek na telefonie); 0 – tylko przełącznik opcji. */
  readonly province = input(0);
  /** Kliknięto (dymek chowa się po otwarciu okna). */
  readonly used = output<void>();

  protected title(): string {
    if (this.province() > 0) return 'Otwórz okno prowincji (i otwieraj je po kliknięciu prowincji)';
    return this.store.provinceWindow()
      ? 'Okno prowincji po kliknięciu: włączone (kliknij – wyłącz)'
      : 'Okno prowincji po kliknięciu: wyłączone (kliknij – włącz)';
  }

  protected toggle(): void {
    const province = this.province();
    if (province > 0) {
      this.store.setProvinceWindow(true);
      this.store.selectedProvince.set(province);
    } else {
      this.store.setProvinceWindow(!this.store.provinceWindow());
    }
    this.used.emit();
  }
}
