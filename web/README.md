# web – frontend (Angular 22 + Pixi)

Dokumentacja całego projektu, w tym uruchamiania frontendu, jest w [README w katalogu głównym](../README.md).

Najważniejsze skrypty (z tego katalogu):

- `npm run prep` – typy TS z Rusta i pakiet wasm (po każdej zmianie w Ruście),
- `npm start` – serwer deweloperski na `http://localhost:4200` (proxy `/ws` do serwera Rust),
- `npm run build` – build produkcyjny do `dist/web` (bez panelu debugu).
