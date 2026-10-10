import { Injectable, signal } from '@angular/core';

import type { ClientMsg } from '../../generated/ClientMsg';
import type { ServerMsg } from '../../generated/ServerMsg';

export type ConnectionStatus = 'connecting' | 'online' | 'offline';

/**
 * Ścieżka WebSocketu dla kolejnej próby połączenia. W trybie otwartym stała (`/ws`); z lobby –
 * `/ws?ticket=…` z nowym biletem przy każdej próbie (bilet jest jednorazowy i ważny 60 s).
 */
export type PathProvider = () => Promise<string>;

const RETRY_MS = 5000;

/** Połączenie WebSocket z serwerem-przekaźnikiem tur. Wiadomości obsługuje `GameSession`. */
@Injectable({ providedIn: 'root' })
export class Transport {
  private socket?: WebSocket;
  private retry?: ReturnType<typeof setTimeout>;
  private path: PathProvider = async () => '/ws';
  /** Rośnie przy każdym `connect`/`disconnect` – stare gniazdo nie wznawia połączenia. */
  private generation = 0;

  readonly status = signal<ConnectionStatus>('offline');
  /** Nazwa gracza wysyłana w `Join` (serwer z biletem i tak zna ją z biletu). */
  playerName = 'gość';
  /** Wiadomość z serwera. */
  onMessage?: (msg: ServerMsg) => void;
  /** Połączenie zamknięte albo nieudane (kolejna próba za 5 s). */
  onClose?: () => void;

  /** Łączy (albo przełącza na inny pokój) – poprzednie połączenie jest zamykane bez ponawiania. */
  connect(path?: PathProvider): void {
    if (path) this.path = path;
    this.closeSocket();
    const generation = ++this.generation;
    this.status.set('connecting');
    void this.open(generation);
  }

  /** Rozłącza i nie ponawia. */
  disconnect(): void {
    this.generation++;
    this.closeSocket();
    this.status.set('offline');
  }

  send(msg: ClientMsg): void {
    if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(JSON.stringify(msg));
  }

  private async open(generation: number): Promise<void> {
    let path: string;
    try {
      path = await this.path();
    } catch {
      // Np. lobby nie wydało biletu (pokój zamknięty, sieć) – jak nieudane połączenie.
      this.lost(generation);
      return;
    }
    if (generation !== this.generation) return;
    this.status.set('connecting');
    const protocol = location.protocol === 'https:' ? 'wss' : 'ws';
    const socket = new WebSocket(`${protocol}://${location.host}${path}`);
    this.socket = socket;
    socket.onopen = () => {
      this.status.set('online');
      this.send({ type: 'join', name: this.playerName });
    };
    socket.onmessage = ({ data }) => this.onMessage?.(JSON.parse(data as string) as ServerMsg);
    socket.onclose = () => this.lost(generation);
  }

  private lost(generation: number): void {
    if (generation !== this.generation) return;
    this.status.set('offline');
    this.retry = setTimeout(() => {
      if (generation === this.generation) void this.open(generation);
    }, RETRY_MS);
    this.onClose?.();
  }

  private closeSocket(): void {
    clearTimeout(this.retry);
    if (this.socket) {
      this.socket.onclose = null;
      this.socket.close();
      this.socket = undefined;
    }
  }
}
