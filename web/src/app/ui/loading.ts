import { Component, computed, inject } from '@angular/core';

import { MapStore } from '../game/map-store';

/**
 * Napis ładowania na środku ekranu z kręcącym się kółkiem. Nie blokuje myszy – mapę można
 * oglądać, gdy dochodzą kolejne warstwy (teren → rysowanie → prowincje).
 */
@Component({
  selector: 'app-loading',
  template: `
    <div class="loading" [class.visible]="label()" role="status" aria-live="polite">
      <span class="spinner" aria-hidden="true"></span>
      <span>{{ label() ?? lastLabel }}</span>
    </div>
  `,
  styleUrl: './ui.css',
})
export class Loading {
  private readonly store = inject(MapStore);
  /** Ostatni napis – zostaje w trakcie wygaszania, żeby tekst nie znikał przed ramką. */
  protected lastLabel = '';

  protected readonly label = computed(() => {
    const label = this.store.busy() || (!this.store.map() && !this.store.error())
      ? 'Generowanie mapy…'
      : this.store.painting()
        ? 'Rysowanie mapy…'
        : this.store.provincesPending()
          ? 'Wyznaczanie prowincji…'
          : null;
    if (label) this.lastLabel = label;
    return label;
  });
}
