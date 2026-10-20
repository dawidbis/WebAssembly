import { Component, computed, inject } from '@angular/core';

import type { FrontSide } from '../../generated/FrontSide';
import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { flagDataUrl } from '../render/flags';
import { RESOURCES } from '../render/resources';
import { full, short } from './format';
import { GoodsIcon } from './goods-icon';

/** Front bez zdobyczy i posiłków kończy się po tylu dniach (`war::STALL_TICKS` / 10 tur na dzień). */
const STALL_DAYS = 30;
/** Zastój widać od tylu dni. */
const STALL_SHOWN_DAYS = 5;
/** Najmniejszy rozmiar panelu przy zmianie uchwytem (px). */
const MIN_W = 240;
const MIN_H = 120;

/**
 * Aktywne fronty (prawy dolny róg, z minimalizacją i zmianą rozmiaru uchwytem w lewym górnym rogu): ataki gracza i ataki na jego prowincje – strony
 * (flaga, wojsko, zaopatrzenie w użyciu), prowincja, % okupacji, „Idź do” – płynny przelot kamery
 * na środek frontu.
 */
@Component({
  selector: 'app-fronts-panel',
  imports: [GoodsIcon],
  templateUrl: './fronts-panel.html',
  styleUrls: ['./ui.css', './fronts-panel.css'],
  host: { '[style.width]': 'width()' },
})
export class FrontsPanel {
  /** Konie – numer surowca (ikona zamiennika paliwa). */
  protected readonly horses = RESOURCES.findIndex((r) => r.name === 'Konie');
  protected readonly store = inject(MapStore);
  protected readonly session = inject(GameSession);

  protected readonly view = computed(() => {
    const fronts = this.session.fronts();
    if (fronts.length === 0) return [];
    const nations = this.session.owners()?.nations ?? [];
    /** Siła zaopatrzenia (×) i paliwo frontu dla podpowiedzi i znaczków. */
    const gear = (strength: number, fuel: number, horses = false) => ({
      strength: `×${Number(strength.toFixed(2)).toString().replace('.', ',')}`,
      strengthTitle: `Siła wojska frontu × ${strength.toFixed(2).replace('.', ',')} – mnożnik ataku z zaopatrzenia wojskowego (bez zaopatrzenia ×0,25; lekkie, standardowe, ciężkie – więcej, wyżej z centrum dowodzenia)`,
      fuel: Math.round(fuel * 100),
      fuelTitle: fuel > 0
        ? `Paliwo: pokrycie ${Math.round(fuel * 100)}% potrzeby – przyspiesza front (do +50%)${horses ? '; brakujące zastępują konie' : ''}`
        : 'Bez paliwa – front wolniejszy',
      horses,
    });
    const side = (s: FrontSide) => {
      const nation = nations.indexOf(s.player) + 1;
      return {
        name: this.session.playerName(s.player),
        flag: nation > 0 ? flagDataUrl(nation, 18, 12) : '',
        troops: short(s.troops),
        troopsTitle: `${full(s.troops)} wojska`,
        supply: s.supply,
        tiles: s.tiles,
      };
    };
    const provinces = this.store.map()?.provinces;
    const name = (p: number) => provinces?.[p - 1]?.name || `#${p}`;
    return fronts.map((f) => {
      const area = f.provinces.reduce((s, p) => s + (provinces?.[p - 1]?.area ?? 0), 0);
      const o = f.opponent;
      return {
      key: `${f.provinces.join(',')}:${f.attacker.player}`,
      province: f.provinces[0] ?? 0,
      provinces: f.provinces,
      name: f.provinces.map(name).join(', '),
      landing: f.landing,
      expedition: f.expedition,
      natives: f.natives,
      lost: short(f.lost),
      sailing: f.sailingDays > 0 ? `${f.crossing ? 'przeprawa' : 'w rejsie'} · ${Math.ceil(f.sailingDays)} d` : '',
      crossing: f.crossing === 'river' ? '≈' : f.crossing === 'mountain' ? '▲' : '',
      crossingTitle:
        f.crossing === 'river'
          ? 'Przeprawa przez rzekę – wojsko na froncie najwyżej do limitu przeprawy (także posiłki)'
          : 'Przeprawa przez góry – wojsko na froncie najwyżej do limitu przeprawy (także posiłki)',
      pace: `×${f.pace.toFixed(1).replace('.', ',')}`,
      mine: f.mine,
      defense: f.defense,
      attacker: side(f.attacker),
      gear: gear(f.strength, f.fuel, f.horses),
      defenders: f.defenders.map(side),
      neutral: f.neutralTiles,
      pocket: f.pocketTiles,
      // Zastój: ile dni do automatycznego końca (0 – front żyje).
      stall: f.idleDays >= STALL_SHOWN_DAYS ? Math.max(1, Math.ceil(STALL_DAYS - f.idleDays)) : 0,
      occupation: Math.round(f.occupation * 100),
      // Wspólny front: przeciwny front (jego armia, straty, tempo, udział w kaflach prowincji).
      opponent: o
        ? {
            ...side(o.side),
            gear: gear(o.strength, o.fuel),
            lost: short(o.lost),
            pace: `×${o.pace.toFixed(1).replace('.', ',')}`,
            share: area > 0 ? Math.round((o.side.tiles / area) * 100) : 0,
          }
        : null,
      x: f.centerX,
      y: f.centerY,
      area: area || 400,
      };
    });
  });

  /** Garnizony odciętych przyczółków gracza. */
  private readonly garrisonView = computed(() => {
    const provinces = this.store.map()?.provinces;
    return this.session.garrisons().map((g) => ({
      key: `${g.x}:${g.y}`,
      name: provinces?.[g.province - 1]?.name || `#${g.province}`,
      troops: short(g.troops),
      title: `${full(g.troops)} wojska w przyczółku`,
      x: g.x,
      y: g.y,
    }));
  });

  /** Panel: fronty i garnizony przyczółków (null – nic do pokazania). */
  protected readonly panel = computed(() => {
    const fronts = this.view();
    const garrisons = this.garrisonView();
    return fronts.length > 0 || garrisons.length > 0 ? { fronts, garrisons } : null;
  });

  /** Udział wojska z suwaka panelu ataku (%). */
  protected share(): number {
    return this.store.attackShare();
  }

  /** Kliknięcie frontu: jego prowincje zaznaczone, panel ataku rozwinięty. */
  protected open(province: number, provinces: number[]): void {
    this.store.attackTargets.set(provinces.length > 1 ? provinces : []);
    this.store.selectedProvince.set(province);
    this.store.setAttackMin(false);
  }

  /** Dosłanie wojska na front – udział z suwaka panelu ataku. */
  protected reinforce(provinces: number[], event: Event): void {
    event.stopPropagation();
    this.session.attack(provinces, this.store.attackShare());
  }

  protected goTo(x: number, y: number, area: number): void {
    this.store.flyTo(Math.round(x), Math.round(y), area);
  }

  /** Szerokość z uchwytu (bez – domyślna z CSS). */
  protected readonly width = computed(() => {
    const size = this.store.frontsSize();
    return size ? `min(${size.w}px, calc(100vw - 16px))` : null;
  });

  /** Uchwyt w lewym górnym rogu: panel rośnie w lewo i w górę (prawy dolny róg stoi w miejscu). */
  protected resize(down: PointerEvent): void {
    const handle = down.currentTarget as HTMLElement;
    const panel = handle.parentElement as HTMLElement;
    handle.setPointerCapture(down.pointerId);
    const start = panel.getBoundingClientRect();
    const move = (e: PointerEvent) => {
      const w = Math.round(Math.max(MIN_W, Math.min(innerWidth - 16, start.width - (e.clientX - down.clientX))));
      const h = Math.round(Math.max(MIN_H, Math.min(innerHeight - 80, start.height - (e.clientY - down.clientY))));
      this.store.frontsSize.set({ w, h });
    };
    const up = () => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', up);
      handle.removeEventListener('pointercancel', up);
      const size = this.store.frontsSize();
      if (size) this.store.setFrontsSize(size);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', up);
    handle.addEventListener('pointercancel', up);
    down.preventDefault();
  }

  protected resetSize(): void {
    this.store.frontsSize.set(null);
    try {
      localStorage.removeItem('mapa.frontsSize');
    } catch {
      // Bez storage.
    }
  }

  protected toggle(): void {
    this.store.frontsPanel.update((s) => (s === 'open' ? 'min' : 'open'));
  }
}
