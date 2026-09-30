import { Injectable, signal } from '@angular/core';

import type { ClientMsg } from '../../generated/ClientMsg';
import type { ServerMsg } from '../../generated/ServerMsg';

export type ConnectionStatus = 'connecting' | 'online' | 'offline';

/** Połączenie WebSocket z serwerem-przekaźnikiem tur. Wiadomości obsługuje `GameSession`. */
@Injectable({ providedIn: 'root' })
export class Transport {
  private socket?: WebSocket;
  private retry?: ReturnType<typeof setTimeout>;

  readonly status = signal<ConnectionStatus>('offline');
  /** Wiadomość z serwera. */
  onMessage?: (msg: ServerMsg) => void;
  /** Połączenie zamknięte albo nieudane (kolejna próba za 5 s). */
  onClose?: () => void;

  connect(): void {
    clearTimeout(this.retry);
    const protocol = location.protocol === 'https:' ? 'wss' : 'ws';
    const socket = new WebSocket(`${protocol}://${location.host}/ws`);
    this.socket = socket;
    this.status.set('connecting');

    socket.onopen = () => {
      this.status.set('online');
      this.send({ type: 'join', name: 'dev' });
    };
    socket.onmessage = ({ data }) => this.onMessage?.(JSON.parse(data as string) as ServerMsg);
    socket.onclose = () => {
      this.status.set('offline');
      this.retry = setTimeout(() => this.connect(), 5000);
      this.onClose?.();
    };
  }

  send(msg: ClientMsg): void {
    if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(JSON.stringify(msg));
  }
}
