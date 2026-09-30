import {
  Component,
  DestroyRef,
  ElementRef,
  ViewContainerRef,
  afterNextRender,
  effect,
  inject,
  untracked,
  viewChild,
} from '@angular/core';

import { MapStore } from './game/map-store';
import { Transport } from './game/transport';
import { MapRenderer } from './render/map-renderer';

@Component({
  selector: 'app-root',
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App {
  private readonly stage = viewChild.required<ElementRef<HTMLElement>>('stage');
  private readonly debugHost = viewChild.required('debugHost', { read: ViewContainerRef });
  private readonly store = inject(MapStore);
  private readonly renderer = new MapRenderer();

  constructor() {
    afterNextRender(async () => {
      await this.renderer.init(this.stage().nativeElement);
      if (DEV_TOOLS) {
        // W buildzie produkcyjnym DEV_TOOLS = false → esbuild wycina ten import w całości.
        const { DebugPanel } = await import('./debug/debug-panel');
        this.debugHost().createComponent(DebugPanel);
      }
    });

    effect(() => {
      const map = this.store.map();
      if (map) this.renderer.setMap(map);
    });
    effect(() => this.renderer.setChunkGridVisible(this.store.showChunkGrid()));
    effect(() => this.renderer.setContours(this.store.showContours()));
    effect(() => this.renderer.setWaves(this.store.showWaves(), this.store.waves()));
    effect(() => this.renderer.setView(
        this.store.showFertility() ? 'fertility' : this.store.showBiomeMap() ? 'biomes' : 'terrain',
      ));
    effect(() => {
      this.store.fitRequest();
      untracked(() => this.renderer.fit());
    });

    void this.store.init();
    inject(Transport).connect();
    inject(DestroyRef).onDestroy(() => this.renderer.destroy());
  }
}
