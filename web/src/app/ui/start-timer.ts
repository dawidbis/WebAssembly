import { Component, DestroyRef, computed, effect, inject, signal } from '@angular/core';

import { GameSession } from '../game/game-session';

/** Czas wyboru w ms (`SELECTION_TICKS` × tura 100 ms w `core/src/protocol.rs`). */
const SELECTION_MS = 20_000;
/** Tura w ms (`TURN_MS` w `server/src/room.rs`). */
const TURN_MS = 100;

/**
 * Odliczanie wyboru prowincji startowej pod paskiem rodzajów map: pasek znika od obu brzegów do
 * środka, na środku czas (00:20). Czas liczy się z ticków gry (koniec wyboru to tick, ten sam
 * u wszystkich), a między turami płynnie dopowiada go zegar przeglądarki – serwer nie wysyła czasu.
 */
@Component({
  selector: 'app-start-timer',
  template: `
    @if (session.phase() === 'waiting' && session.showsGameMap()) {
      <p class="frame timer-note">Czekanie, aż wszyscy gracze wczytają mapę…</p>
    } @else if (view(); as v) {
      <div class="frame timer" role="timer" [attr.aria-label]="'Wybór prowincji startowej: ' + v.label">
        <div class="track">
          <div class="fill" [style.transform]="'scaleX(' + v.fraction + ')'"></div>
          <span class="time" [class.urgent]="v.urgent">{{ v.label }}</span>
        </div>
        <p class="hint">{{ v.hint }}</p>
      </div>
    }
  `,
  styleUrls: ['./ui.css'],
})
export class StartTimer {
  protected readonly session = inject(GameSession);
  /** Bieżący czas (`performance.now()`), odświeżany co klatkę w trakcie odliczania. */
  private readonly now = signal(performance.now());
  private frame = 0;

  protected readonly view = computed(() => {
    const s = this.session.selection();
    if (!s || !this.session.showsGameMap()) return null;
    const left = Math.max(0, (s.end - s.tick) * TURN_MS);
    const remaining = Math.max(0, Math.min(left, left - (this.now() - s.at)));
    const seconds = Math.ceil(remaining / 1000);
    const start = this.myStart();
    return {
      fraction: Math.min(1, remaining / SELECTION_MS),
      label: `${String(Math.floor(seconds / 60)).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`,
      urgent: seconds <= 5,
      hint: start
        ? `Twój start: prowincja ${start} – kliknij inną wolną, aby zmienić`
        : 'Kliknij wolną prowincję, aby wybrać start (bez wyboru – losowy)',
    };
  });

  /** Prowincja startowa tego gracza (jedyna prowincja z jego państwem w czasie wyboru). */
  private readonly myStart = computed(() => {
    const nation = this.session.myNation();
    const shares = this.session.owners()?.shares;
    if (!nation || !shares) return null;
    const k = shares.findIndex((list) => list.some(([n]) => n === nation));
    return k >= 0 ? k + 1 : null;
  });

  constructor() {
    // Klatki tylko w czasie odliczania.
    effect(() => {
      cancelAnimationFrame(this.frame);
      if (!this.session.selection()) return;
      const loop = () => {
        this.now.set(performance.now());
        this.frame = requestAnimationFrame(loop);
      };
      loop();
    });
    inject(DestroyRef).onDestroy(() => cancelAnimationFrame(this.frame));
  }
}
