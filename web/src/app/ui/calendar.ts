import { Component, computed, inject } from '@angular/core';

import { GameSession } from '../game/game-session';

/**
 * Upływ czasu w grze: jedna pora dnia (6 godzin) co tyle tur – 25 tur = 2,5 s, dzień = 10 s,
 * miesiąc ok. 5 min, rok ok. 1 h. Na razie tylko na ekranie (rdzeń liczy w turach); gdy kalendarz
 * zacznie wpływać na grę (pory roku), przenieś to do rdzenia.
 */
export const TICKS_PER_QUARTER = 25;
/** Gra zaczyna się 1 stycznia roku 0 o 06:00 (pora dnia nr 1). */
const START_QUARTER = 1;
const MONTHS = ['sty', 'lut', 'mar', 'kwi', 'maj', 'cze', 'lip', 'sie', 'wrz', 'paź', 'lis', 'gru'];
const MONTH_NAMES = [
  'stycznia', 'lutego', 'marca', 'kwietnia', 'maja', 'czerwca',
  'lipca', 'sierpnia', 'września', 'października', 'listopada', 'grudnia',
];
const MONTH_DAYS = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const YEAR_DAYS = 365;

/** Data gry po `ticks` turach od początku gry właściwej. */
export function gameDate(ticks: number): { year: number; month: number; day: number; hour: number } {
  const quarter = Math.floor(ticks / TICKS_PER_QUARTER) + START_QUARTER;
  const days = Math.floor(quarter / 4);
  let rest = days % YEAR_DAYS;
  let month = 0;
  while (rest >= MONTH_DAYS[month]) rest -= MONTH_DAYS[month++];
  return { year: Math.floor(days / YEAR_DAYS), month, day: rest + 1, hour: (quarter % 4) * 6 };
}

/** Kalendarz w prawym górnym rogu: dzień, miesiąc, rok i pora dnia (00:00 / 06:00 / 12:00 / 18:00). */
@Component({
  selector: 'app-calendar',
  template: `
    @if (date(); as d) {
      <div class="frame calendar" [class.waiting]="!d.running" [title]="d.title">
        <svg class="sky" viewBox="0 0 24 24" aria-hidden="true">
          @if (d.night) {
            <path d="M18.5 15.5A7.5 7.5 0 0 1 8.5 5.5a7.5 7.5 0 1 0 10 10Z" />
          } @else {
            <circle cx="12" cy="12" r="4" />
            <path d="M12 2.5v2.5M12 19v2.5M2.5 12H5M19 12h2.5M5.3 5.3l1.8 1.8M16.9 16.9l1.8 1.8M5.3 18.7l1.8-1.8M16.9 7.1l1.8-1.8" />
          }
        </svg>
        <span class="date">{{ d.day }} {{ d.month }} <small>rok {{ d.year }}</small></span>
        <span class="time">{{ d.hour }}</span>
      </div>
    }
  `,
  styleUrls: ['./ui.css'],
})
export class Calendar {
  private readonly session = inject(GameSession);

  protected readonly date = computed(() => {
    const tick = this.session.tick();
    if (tick === null || !this.session.showsGameMap()) return null;
    const since = this.session.playingSince();
    // Przed grą właściwą (wybór startu) – data startowa, przygaszona.
    const d = gameDate(since === null ? 0 : Math.max(0, tick - since));
    const hour = `${String(d.hour).padStart(2, '0')}:00`;
    return {
      running: since !== null,
      day: d.day,
      month: MONTHS[d.month],
      year: d.year,
      hour,
      night: d.hour === 0 || d.hour === 18,
      title: `${d.day} ${MONTH_NAMES[d.month]} roku ${d.year}, ${hour}`,
    };
  });
}
