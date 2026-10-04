import { Component, inject, signal } from '@angular/core';

import { MapStore } from '../game/map-store';
import type { TerrainView } from '../render/terrain';

/** Rodzaje mapy z klawiszami 1, 2, 3, 4 (kolejność przycisków). */
const VIEWS: { view: TerrainView; label: string; key: string; code: string }[] = [
  { view: 'terrain', label: 'Teren', key: '1', code: 'Digit1' },
  { view: 'political', label: 'Polityczna', key: '2', code: 'Digit2' },
  { view: 'biomes', label: 'Biomy', key: '3', code: 'Digit3' },
  { view: 'fertility', label: 'Żyzność', key: '4', code: 'Digit4' },
];

/**
 * Górny pasek dla każdego gracza: dopasowanie widoku, rodzaje mapy (zawsze widoczne, na środku)
 * i opcje renderowania schowane pod zębatką. Obsługuje też skróty klawiszowe.
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
