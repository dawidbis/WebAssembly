# GDD – gospodarka (surowce, zaopatrzenie, budynki, rynek)

Dokument projektowy gospodarki i ekspansji. Opisuje **docelowe zasady**; przy każdej sekcji zaznaczamy, co już jest w kodzie (✅) a co czeka (⏳) – pełna lista braków jest w [planie](plan.md#czego-jeszcze-nie-ma). Liczby oznaczone *(do strojenia)* to propozycje startowe: zmieniamy je po pierwszych grach testowych.

Dyplomacja jest na razie poza zakresem.

## Spis

| Plik | Co opisuje |
|---|---|
| [surowce.md](surowce.md) | 39 surowców prowincji, budynki wydobycia (typ dedukowany z surowca), wydobycie bazowe, ceny bazowe, wyjątek złota |
| [zaopatrzenie.md](zaopatrzenie.md) | rodzaje zaopatrzenia (wojskowe, cywilne, paliwo), receptury, fabryki (wybór wejść), walka (fronty, armia tubylcza, paliwo według zaopatrzenia), podatki |
| [budynki.md](budynki.md) | budynki w prowincjach (poziomy, budynek kontroluje właściciel jego kafla), miejsca na mapie, infrastruktura (punkty na budynki), koszty (złoto + materiały inżynieryjne) i czas budowy, obsada robotnikami, stolica i aneksja, centrum dowodzenia 1–6 (fronty, desanty, umocnienia, zaopatrzenie), miasto, uniwersytet, umocnienia, port |
| [rynek.md](rynek.md) | automatyczny rynek surowców: suwaki bilansu import/eksport, pojemność, cena, transport lądem i morzem, handlarze spoza mapy |
| [balans.md](balans.md) | pomiar rozkładu surowców i popytu, cele i plan rewizji balansu (surowce, receptury, zużycie) |
| [plan.md](plan.md) | kolejność wdrażania w kodzie, stan i lista mechanik, których jeszcze nie ma |

## Przepływ w skrócie

```
prowincja (stały surowiec)
   │  wydobycie: 0,10/dzień bez budynku, +1,0/dzień za poziom budynku wydobycia (× obsada robotnikami)
   ▼
globalna pula surowców gracza  ──────────────►  rynek (później): kupno / sprzedaż
   │  fabryki (dowolna prowincja, 1 rodzaj na fabrykę, 1,0 szt./dzień za poziom × obsada)
   ▼
globalna pula zaopatrzenia gracza
   ├─ wojskowe (lekkie / standardowe / ciężkie) ─► siła wojska w walce
   ├─ paliwo ─────────────────────────────────► szybkość zajmowania
   └─ cywilne: podstawowe ────────────────────► warunek premii z luksusowego
               luksusowe (przy pełnym podst.) ──► premia do podatków
   └─ prowiant (z surowców spożywczych) ──────► przyrost populacji / głód
```

Złoto z kopalni złota omija pulę i rynek – trafia od razu do skarbca.

## Słownik

| Pojęcie | Znaczenie |
|---|---|
| **Państwo** | terytorium gracza (kafle z jego numerem państwa, `core/src/owners.rs`) |
| **Stolica** | prowincja startowa gracza – w niej stoi centrum dowodzenia |
| **Surowiec** | stały zasób prowincji (`Province.resource`, jeden na prowincję) |
| **Zaopatrzenie** | dobra wytwarzane w fabrykach z surowców |
| **Pula** | globalny zapas gracza (surowców i zaopatrzenia) – bez lokalnych magazynów i transportu |
| **Pokrycie** | ile potrzeby jest zaspokojone: `min(1, dostępne / potrzebne)` – np. obsada budynków, zaopatrzenie cywilne |
| **Wojsko / robotnicy** | dwie części populacji (suwak w pasku zasobów); wojsko walczy, robotnicy obsadzają budynki i płacą podatki |

## Jednostki czasu i liczby

- **Tura** = 100 ms. **Dzień gry** = ok. 1 s (miesiąc ma prawdziwą liczbę dni). **Miesiąc** = 30 s = 300 tur. **Rok** = 6 min. Kalendarz: `web/src/app/ui/calendar.ts`.
- Produkcja i zużycie podajemy **na dzień**; rdzeń liczy dzień jako równe **10 tur** (`goods::TICKS_PER_DAY`) i rozlicza co turę w tysięcznych częściach jednostki (kalendarz na ekranie dzieli miesiąc na prawdziwą liczbę dni, więc jego dzień to 9,7–10,7 tury).
- **Złoto** w rdzeniu jest w jednostkach surowych; interfejs pokazuje je **podzielone przez 10**. Ceny i koszty w tym dokumencie podajemy w złocie **z interfejsu** (tak jak widzi je gracz).
- W rdzeniu wszystkie ilości są **liczbami całkowitymi** (np. tysięczne części jednostki – tak jak populacja w `core/src/economy.rs`), żeby gra była identyczna u wszystkich graczy (determinizm, hash stanu).

## Zasady techniczne (dla implementacji)

- Każda zasada gry żyje w **rdzeniu** (`crates/core`), a nie w interfejsie; stan (zapasy, budynki, kolejki budowy) jest polem `Game` inicjalizowanym w `from_map` i dopisanym do `state_hash`.
- Działania gracza to **intencje** (`Intent::Build`, `Intent::SetFactory` …) walidowane w rdzeniu – serwer tylko stempluje gracza.
- Stałe zasad gry są na górze modułów rdzenia; nie mają suwaków w panelu debugu, bo muszą być takie same u wszystkich graczy.
