import { Component, computed, inject } from '@angular/core';

import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';

/** Promień rozmieszczenia przycisków wokół kursora (px). */
const RADIUS = 46;

interface Item {
  id: 'buildings' | 'attack' | 'center';
  label: string;
  icon: string;
}

/**
 * Menu kołowe po kliknięciu prawym przyciskiem w prowincję: własna – budynki, cudza albo niczyja – atak;
 * zawsze – wyśrodkowanie kamery. Wybór zaznacza prowincję (otwiera okno budynków albo panel ataku).
 * Zamyka je kliknięcie obok albo Esc.
 */
@Component({
  selector: 'app-radial-menu',
  host: { '(document:pointerdown)': 'onOutside($event)', '(document:keydown.escape)': 'close()' },
  template: `
    @if (view(); as v) {
      <div class="radial" [style.left.px]="v.x" [style.top.px]="v.y" role="menu" [attr.aria-label]="'Prowincja ' + v.name">
        <span class="name">{{ v.name }}</span>
        @for (it of v.items; track it.id) {
          <button
            type="button"
            role="menuitem"
            [class]="'item ' + it.id"
            [style.left.px]="it.dx"
            [style.top.px]="it.dy"
            [title]="it.label"
            [attr.aria-label]="it.label"
            (click)="choose(it.id, v.province)"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true"><path [attr.d]="it.icon" /></svg>
          </button>
        }
      </div>
    }
  `,
  styleUrl: './radial-menu.css',
})
export class RadialMenu {
  private readonly store = inject(MapStore);
  private readonly session = inject(GameSession);

  protected readonly view = computed(() => {
    const m = this.store.contextMenu();
    if (!m) return null;
    const items: Item[] = [];
    if (this.isMine(m.province)) {
      items.push({ id: 'buildings', label: 'Budynki', icon: 'M3 20V10l5 3V10l5 3V6h3v14zM17 6h3v14' });
    } else if (this.session.myNation()) {
      items.push({ id: 'attack', label: 'Atak', icon: 'M5 19 19 5M15 5h4v4M5 15l4 4M3 21l3-3' });
    }
    items.push({ id: 'center', label: 'Wyśrodkuj', icon: 'M12 3v4M12 17v4M3 12h4M17 12h4M12 9a3 3 0 1 0 0 6 3 3 0 1 0 0-6' });
    // Przyciski na okręgu, od góry zgodnie z ruchem wskazówek.
    const placed = items.map((it, k) => {
      const angle = -Math.PI / 2 + (k * 2 * Math.PI) / Math.max(3, items.length);
      return { ...it, dx: Math.cos(angle) * RADIUS, dy: Math.sin(angle) * RADIUS };
    });
    const name = this.store.map()?.provinces[m.province - 1]?.name || `#${m.province}`;
    return { x: m.x, y: m.y, province: m.province, name, items: placed };
  });

  /** Czy prowincja należy do gracza (ponad 50% kafli – jak `Owners::province_owner`). */
  private isMine(province: number): boolean {
    const nation = this.session.myNation();
    const area = this.store.map()?.provinces[province - 1]?.area ?? 0;
    const shares = this.session.owners()?.shares[province - 1] ?? [];
    return !!nation && shares.some(([n, tiles]) => n === nation && tiles * 2 > area);
  }

  protected choose(id: Item['id'], province: number): void {
    this.close();
    if (id === 'center') {
      const p = this.store.map()?.provinces[province - 1];
      if (p) this.store.flyTo(p.centerX, p.centerY);
      return;
    }
    this.store.buildingsClosedFor.set(0);
    this.store.selectedProvince.set(province);
  }

  protected close(): void {
    this.store.contextMenu.set(null);
  }

  protected onOutside(event: PointerEvent): void {
    if (this.store.contextMenu() && !(event.target as HTMLElement).closest('app-radial-menu')) this.close();
  }
}
