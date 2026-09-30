import { Component, computed, inject } from '@angular/core';

import { GameSession } from '../game/game-session';
import { Transport } from '../game/transport';

/**
 * Komunikat pod górnym paskiem – tylko gdy z grą jest problem: rozbieżność stanu (desync),
 * gra zatrzymana błędem albo utracone połączenie z serwerem. Bez serwera od początku (tryb
 * lokalny, np. strojenie generatora) nic nie pokazuje.
 */
@Component({
  selector: 'app-game-status',
  template: `
    @if (notice(); as notice) {
      <p class="frame notice" role="alert">{{ notice }}</p>
    }
  `,
  styleUrl: './ui.css',
})
export class GameStatus {
  private readonly session = inject(GameSession);
  private readonly transport = inject(Transport);

  protected readonly notice = computed(() => {
    const error = this.session.error();
    if (error) return `Gra zatrzymana: ${error}`;
    const desync = this.session.desync();
    if (desync !== null) return `Stan gry rozjechał się z innymi graczami (tura ${desync}) – odśwież stronę`;
    if (this.session.config() && this.transport.status() !== 'online') return 'Brak połączenia z serwerem – ponawiam…';
    return null;
  });
}
