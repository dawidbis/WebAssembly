import { Component, computed, inject } from '@angular/core';

import { Lobby } from '../game/lobby';
import { MapStore } from '../game/map-store';
import { Transport } from '../game/transport';

/**
 * Napis ładowania na środku ekranu z kręcącym się kółkiem. Nie blokuje myszy – mapę można
 * oglądać, gdy dochodzą kolejne warstwy (łączenie → teren → rysowanie → prowincje).
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
  private readonly transport = inject(Transport);
  private readonly lobby = inject(Lobby);
  /** Ostatni napis – zostaje w trakcie wygaszania, żeby tekst nie znikał przed ramką. */
  protected lastLabel = '';

  protected readonly label = computed(() => {
    // Ekran powitalny / poczekalnia zasłania mapę (i nic się nie generuje przed wejściem do pokoju).
    if (this.lobby.available() !== false && this.lobby.screen() !== 'game') return null;
    const waiting = !this.store.map() && !this.store.error();
    // Mapa powstaje z konfiguracji serwera, więc do `Welcome` nic się jeszcze nie generuje.
    const connecting = waiting && !this.store.busy() && this.transport.status() === 'connecting';
    const label = this.stage(connecting, waiting);
    if (label) this.lastLabel = label;
    return label;
  });

  /** Napis bieżącego etapu wczytywania (null = nic się nie dzieje). */
  private stage(connecting: boolean, waiting: boolean): string | null {
    if (connecting) return 'Łączenie z serwerem…';
    if (this.store.busy() || waiting) return 'Generowanie mapy…';
    if (this.store.painting()) return 'Rysowanie mapy…';
    if (this.store.provincesPending()) return 'Wyznaczanie prowincji…';
    if (this.store.polishPending()) return 'Ostatnie szlify…';
    if (this.store.eggsPending()) return 'Chowanie easter eggów…';
    return null;
  }
}
