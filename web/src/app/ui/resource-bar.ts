import { Component, DestroyRef, computed, effect, inject, signal } from '@angular/core';

import { GameSession } from '../game/game-session';
import { flagDataUrl } from '../render/flags';
import { nationCss } from '../render/nations';
import { full, short } from './format';

/** Złoto w interfejsie jest dzielone przez tyle (GDD). */
const GOLD_UNIT = 100;
/** Tura w ms (`TURN_MS` w `server/src/room.rs`); podatek co 300 tur (`economy::TAX_TICKS`). */
const TURN_MS = 100;
const TAX_MS = 30_000;

/** Dymek z wpływem podatku unosi się i znika po tylu ms (animacja `tax-float` w resource-bar.css). */
const BUBBLE_MS = 1800;

/**
 * Pasek zasobów gracza (środek górnego paska): flaga i nazwa państwa, populacja z limitem
 * i przyrostem, podział nowej populacji na wojsko i robotników (suwak z bezwładnością – rzeczywisty
 * podział idzie za celem w rdzeniu) i złoto z kółkiem postępu do podatku (wpływ – unoszący się dymek).
 * Liczby skrócone (k / m), pełne w podpowiedziach. Widoczny, gdy gracz ma państwo.
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
  /** Dymki z wpływami podatku (każdy żyje `BUBBLE_MS`). */
  protected readonly bubbles = signal<{ id: number; text: string }[]>([]);
  private bubbleId = 0;
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
      population: short(e.population),
      maxPopulation: short(e.maxPopulation),
      fill: fill * 100,
      growth: `${growth >= 0 ? '+' : '−'}${short(Math.abs(growth))}/s`,
      shrinking: growth < 0,
      troops: short(e.troops),
      workers: short(e.workers),
      share: e.troopShare,
      target,
      moving: target !== e.troopShare,
      gold: short(Math.floor(e.gold / GOLD_UNIT)),
      tax: this.taxCountdown(),
      // Pełne liczby w podpowiedziach (na pasku – skrócone k / m).
      popTitle: `Populacja ${full(e.population)} / limit ${full(e.maxPopulation)} (100 osób na kafel). Najszybciej rośnie przy 55% limitu.`,
      troopsTitle: `Wojsko: ${full(e.troops)} – ekspansja i obrona`,
      workersTitle: `Robotnicy: ${full(e.workers)} – produkcja i wydobycie`,
      goldTitle: `Złoto: ${full(Math.floor(e.gold / GOLD_UNIT))}. Następny podatek: +${full(Math.floor(e.nextTax / GOLD_UNIT))} (1 od osoby co 30 s)`,
    };
  });

  constructor() {
    const timer = setInterval(() => this.now.set(performance.now()), 250);
    effect(() => {
      if (this.session.myEconomy()?.targetShare === this.pending()) this.pending.set(null);
    });
    // Podatek wpłynął – zielony dymek „+333” nad złotem, unosi się i znika.
    let lastGold: number | null = null;
    effect(() => {
      const gold = this.session.myEconomy()?.gold ?? null;
      const gain = gold !== null && lastGold !== null ? Math.floor(gold / GOLD_UNIT) - Math.floor(lastGold / GOLD_UNIT) : 0;
      lastGold = gold;
      if (gain <= 0) return;
      const bubble = { id: ++this.bubbleId, text: `+${short(gain)}` };
      this.bubbles.update((list) => [...list, bubble]);
      setTimeout(() => this.bubbles.update((list) => list.filter((b) => b !== bubble)), BUBBLE_MS);
    });
    inject(DestroyRef).onDestroy(() => clearInterval(timer));
  }

  /** Postęp okresu podatku 0..1 – z ticku podatku, płynnie między turami. */
  private taxCountdown(): { progress: number } | null {
    const t = this.session.tax();
    if (!t) return null;
    const left = Math.max(0, (t.at - t.tick) * TURN_MS - (this.now() - t.now));
    return { progress: 1 - left / TAX_MS };
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
