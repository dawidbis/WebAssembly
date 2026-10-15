# Rynek (szkic – etap późniejszy)

Rzadko który gracz będzie miał wszystkie surowce potrzebne do pełnej produkcji – rynek pozwala kupić brakujące i sprzedać nadmiar. Ten dokument to **szkic do dyskusji**; przed wdrożeniem doprecyzujemy go z użytkownikiem.

## Założenia

- **Jeden globalny rynek** dla wszystkich graczy w grze.
- Handluje się **surowcami i zaopatrzeniem**. **Złoto jest poza rynkiem** – wydobyte trafia od razu do skarbca ([surowce.md](surowce.md#złoto)).
- Każde dobro ma **cenę bazową** (surowce – tabela w [surowce.md](surowce.md#ceny-bazowe); zaopatrzenie – suma cen składników × 1,3 *(do strojenia)*).

## Cena

- Cena bieżąca = cena bazowa × (popyt / podaż)^k, przycięta do **[0,25; 4] × cena bazowa** *(k ≈ 0,5, do strojenia)*.
- **Popyt i podaż** to ilości ze zleceń kupna i sprzedaży z ostatnich dni (np. średnia krocząca z 30 dni), z dodaną stałą „płynnością” (rynek „świata zewnętrznego”), żeby cena nie skakała przy małym handlu i dało się kupić coś, czego nikt nie sprzedaje – tylko drożej.
- Cena aktualizowana **raz dziennie** (deterministycznie w rdzeniu – zlecenia to intencje).

## Zlecenia

- Intencje `Buy { good, amount, max_price }` i `Sell { good, amount, min_price }` albo prościej: **stałe zlecenia** („kupuj 2 Rudy żelaza dziennie”, „sprzedawaj nadwyżkę Zbóż ponad 50”) wykonywane codziennie po bieżącej cenie.
- **Prowizja** od transakcji *(np. 10%)* – złoto znika z gry (hamuje inflację).

## Porty i handel morski

- **Bez portu**: handel lądowy z limitem wolumenu na dzień *(np. 5 jednostek)* i pełną prowizją.
- **Port** (prowincja nad morzem, poziomy 1–3): każdy poziom zwiększa limit wolumenu i obniża prowizję – handel morski.
- Później: blokada morska (wróg odcina porty) – po walce na morzu.

## Otwarte pytania

- Stałe zlecenia czy jednorazowe kupno/sprzedaż (albo oba)?
- Czy gracze widzą, kto sprzedaje (anonimowy rynek vs. widoczne oferty)?
- Czy rynek „świata zewnętrznego” ma nieskończoną płynność po wyższej cenie, czy ograniczoną?
