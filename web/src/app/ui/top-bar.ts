import { Component, inject, signal } from '@angular/core';

import { MapStore } from '../game/map-store';
import type { TerritoryStyle } from '../render/map-renderer';
import type { TerrainView } from '../render/terrain';

/** Rodzaje mapy z klawiszami 1–4 (kolejność ikon) i ikoną (ścieżka SVG 24 × 24, obrys). */
const VIEWS: { view: TerrainView; label: string; key: string; code: string; icon: string }[] = [
  { view: 'terrain', label: 'Teren', key: '1', code: 'Digit1', icon: 'M2.5 19.5 9 8l4 6.5 2.5-3.5 6 8.5ZM7 12.5l2 1.5 2-1' },
  { view: 'political', label: 'Polityczna', key: '2', code: 'Digit2', icon: 'M3.5 6 9 3.5l6 2.5 5.5-2.5v14.5L15 20.5 9 18l-5.5 2.5ZM9 3.5V18M15 6v14.5' },
  { view: 'biomes', label: 'Biomy', key: '3', code: 'Digit3', icon: 'M12 21v-8M12 13c-4.5 0-7-3-7-7 4 0 7 2.5 7 7Zm0 0c0-5 3-8 7.5-8 0 5-3 8-7.5 8Z' },
  { view: 'resources', label: 'Zasoby', key: '4', code: 'Digit4', icon: 'M6.5 4h11L21 9.5 12 20.5 3 9.5ZM3 9.5h18M9 4l3 16.5L15 4' },
];

/**
 * Pasek rodzajów mapy (lewy dolny róg, w pionie, nad ramką prowincji): ikony widoków, dopasowanie
 * widoku i opcje renderowania pod zębatką (rozwijane w prawo). Obsługuje też skróty klawiszowe.
 */
@Component({
  selector: 'app-top-bar',
  templateUrl: './top-bar.html',
  styleUrl: './ui.css',
  host: { '(window:keydown)': 'onKey($event)', '(document:pointerdown)': 'onOutside($event)' },
})
export class TopBar {
  protected readonly store = inject(MapStore);
  protected readonly views = VIEWS;
  /** Rozwinięta lista opcji renderowania (zębatka). */
  protected readonly optionsOpen = signal(false);

  protected checked(event: Event): boolean {
    return (event.target as HTMLInputElement).checked;
  }

  protected setOpacity(event: Event): void {
    this.store.borderOpacity.set(Number((event.target as HTMLInputElement).value));
  }

  /** Suwak wyglądu terytoriów (krycie granic, tła, nazw). */
  protected setTerritory(key: 'borderOpacity' | 'fillOpacity' | 'flagOpacity' | 'namesOpacity', event: Event): void {
    const value = Number((event.target as HTMLInputElement).value);
    this.store.territory.update((t) => ({ ...t, [key]: value }));
  }

  protected setFill(fill: TerritoryStyle['fill']): void {
    this.store.territory.update((t) => ({ ...t, fill }));
  }

  protected setNames(showNames: boolean): void {
    this.store.territory.update((t) => ({ ...t, showNames }));
  }

  /** Kliknięcie poza paskiem zwija listę opcji. */
  protected onOutside(event: PointerEvent): void {
    if (this.optionsOpen() && !(event.target as HTMLElement).closest('app-top-bar')) this.optionsOpen.set(false);
  }

  protected onKey(event: KeyboardEvent): void {
    const target = event.target as HTMLElement;
    if (target instanceof HTMLInputElement && (target.type === 'number' || target.type === 'text')) return;
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const view = VIEWS.find((v) => v.code === event.code);
    if (view) {
      this.store.view.set(view.view);
      event.preventDefault();
      return;
    }
    switch (event.code) {
      case 'KeyP':
        this.store.showProvinces.update((v) => !v);
        break;
      case 'KeyI':
        this.store.showContours.update((v) => !v);
        break;
      case 'KeyW':
        this.store.showWaves.update((v) => !v);
        break;
      case 'KeyF':
        this.store.requestFit();
        break;
      default:
        return;
    }
    event.preventDefault();
  }
}
