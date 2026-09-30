import { Component, inject } from '@angular/core';

import { MapStore } from '../game/map-store';
import type { TerrainView } from '../render/terrain';

const VIEWS: { view: TerrainView; label: string; key: string }[] = [
  { view: 'terrain', label: 'Teren', key: '1' },
  { view: 'political', label: 'Polityczna', key: 'M' },
  { view: 'biomes', label: 'Biomy', key: 'B' },
  { view: 'fertility', label: 'Żyzność', key: 'Z' },
];

/** Górny pasek dla każdego gracza: rodzaj mapy i opcje renderowania. Obsługuje też skróty klawiszowe widoku. */
@Component({
  selector: 'app-top-bar',
  templateUrl: './top-bar.html',
  styleUrl: './ui.css',
  host: { '(window:keydown)': 'onKey($event)' },
})
export class TopBar {
  protected readonly store = inject(MapStore);
  protected readonly views = VIEWS;

  /** Klawisz widoku przełącza na dany widok albo z powrotem na teren. */
  protected toggleView(view: TerrainView): void {
    this.store.view.update((v) => (v === view ? 'terrain' : view));
  }

  protected checked(event: Event): boolean {
    return (event.target as HTMLInputElement).checked;
  }

  protected setOpacity(event: Event): void {
    this.store.borderOpacity.set(Number((event.target as HTMLInputElement).value));
  }

  protected onKey(event: KeyboardEvent): void {
    const target = event.target as HTMLElement;
    if (target instanceof HTMLInputElement && (target.type === 'number' || target.type === 'text')) return;
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    switch (event.code) {
      case 'Digit1':
        this.store.view.set('terrain');
        break;
      case 'KeyM':
        this.toggleView('political');
        break;
      case 'KeyB':
        this.toggleView('biomes');
        break;
      case 'KeyZ':
        this.toggleView('fertility');
        break;
      case 'KeyP':
        this.store.showProvinces.update((v) => !v);
        break;
      case 'KeyT':
        this.store.showTrees.update((v) => !v);
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
      case 'Escape':
        this.store.selectedProvince.set(0);
        break;
      default:
        return;
    }
    event.preventDefault();
  }
}
