import { Component, DestroyRef, computed, effect, inject, signal } from '@angular/core';

import { GameSession } from '../game/game-session';
import { flagDataUrl } from '../render/flags';
import { nationCss } from '../render/nations';

/** Złoto w interfejsie jest dzielone przez tyle (GDD). */
const GOLD_UNIT = 100;
/** Tura w ms (`TURN_MS` w `server/src/room.rs`); podatek co 300 tur (`economy::TAX_TICKS`). */
const TURN_MS = 100;
const TAX_MS = 30_000;

const whole = new Intl.NumberFormat('pl-PL', { maximumFractionDigits: 0 });
const compact = new Intl.NumberFormat('pl-PL', { notation: 'compact', maximumFractionDigits: 1 });

/** Liczba po polsku: do miliona pełna (25 000), dalej skrócona (1,2 mln). */
function count(n: number): string {
  return Math.abs(n) < 1_000_000 ? whole.format(n) : compact.format(n);
}

/**
 * Pasek zasobów gracza (lewa część górnego paska): flaga i nazwa państwa, populacja z limitem
 * i przyrostem, podział nowej populacji na wojsko i robotników (suwak z bezwładnością – rzeczywisty
 * podział idzie za celem w rdzeniu) i złoto z odliczaniem do podatku. Widoczny, gdy gracz ma państwo.
 */
@Component({
  selector: 'app-resource-bar',
  templateUrl: './resource-bar.html',
  styleUrls: ['./ui.css', './resource-bar.css'],
})
export class ResourceBar {
  protected readonly session = inject(GameSession);
  /** Cel suwaka w trakcie przeciągania (zanim wróci z rdzenia w raporcie). */
  protected readonly dragging = signal<number | null>(null);
  /** Cel wysłany do rdzenia, a jeszcze niepotwierdzony raportem (suwak nie odskakuje na chwilę). */
  private readonly pending = signal<number | null>(null);
  /** Zegar odliczania podatku (co 250 ms wystarczy – sekundy). */
  private readonly now = signal(performance.now());

  protected readonly view = computed(() => {
    const e = this.session.myEconomy();
    const nation = this.session.myNation();
    if (!e || !nation) return null;
    const target = this.dragging() ?? this.pending() ?? e.targetShare;
    const fill = e.maxPopulation > 0 ? Math.min(1, e.population / e.maxPopulation) : 0;
    const growth = Math.round(e.growthPerSecond);
    return {
      name: this.session.playerName(e.player),
      flag: flagDataUrl(nation, 36, 24),
      color: nationCss(nation),
      population: count(e.population),
      maxPopulation: count(e.maxPopulation),
      fill: fill * 100,
      growth: `${growth >= 0 ? '+' : '−'}${count(Math.abs(growth))}/s`,
      shrinking: growth < 0,
      troops: count(e.troops),
      workers: count(e.workers),
      share: e.troopShare,
      target,
      moving: target !== e.troopShare,
      gold: count(Math.floor(e.gold / GOLD_UNIT)),
      nextTax: count(Math.floor(e.nextTax / GOLD_UNIT)),
      tax: this.taxCountdown(),
    };
  });

  constructor() {
    const timer = setInterval(() => this.now.set(performance.now()), 250);
    effect(() => {
      if (this.session.myEconomy()?.targetShare === this.pending()) this.pending.set(null);
    });
    inject(DestroyRef).onDestroy(() => clearInterval(timer));
  }

  /** Czas do podatku (mm:ss) i postęp okresu 0..1 – z ticku podatku, płynnie między turami. */
  private taxCountdown(): { label: string; progress: number } | null {
    const t = this.session.tax();
    if (!t) return null;
    const left = Math.max(0, (t.at - t.tick) * TURN_MS - (this.now() - t.now));
    const seconds = Math.ceil(left / 1000);
    return { label: `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`, progress: 1 - left / TAX_MS };
  }

  protected drag(event: Event): void {
    this.dragging.set(Number((event.target as HTMLInputElement).value));
  }

  /** Puszczenie suwaka: cel idzie do rdzenia jako intencja (rzeczywisty podział dojdzie z bezwładnością). */
  protected commit(event: Event): void {
    const value = Number((event.target as HTMLInputElement).value);
    this.session.setTroopShare(value);
    this.pending.set(value);
    this.dragging.set(null);
  }
}
