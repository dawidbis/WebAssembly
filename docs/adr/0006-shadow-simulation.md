# 0006. Symulacja-cień na serwerze jako anticheat lockstepu

**Status:** przyjęta (2026-10), do wdrożenia w etapie 2b

## Kontekst

Game-server jest dziś przekaźnikiem (`crates/server/src/room.rs`): zegar tur, stemplowanie intencji, log tur, porównanie hashy stanu. Prawdą jest **pierwszy zgłoszony** hash dla ticka – klient, który zgłosi zmanipulowany hash jako pierwszy, sprawia, że uczciwi gracze dostają `Desync`.

Rdzeń (`game_core::Game`) jest deterministyczny i daje ten sam wynik natywnie i w wasm (pilnowane testami i tabelą hashy w README).

## Decyzja

Game-server prowadzi natywnie własną kopię gry („cień”), wykonuje każdą turę zaraz po jej rozesłaniu i:

1. jego `state_hash` jest autorytatywny – klient z innym hashem dostaje `Desync`, po progu jest wyrzucany,
2. wysyła w `Welcome` hashe mapy (teren, biomy, lasy, prowincje); klient porównuje je z mapą wygenerowaną w wasm przed startem gry (zmodyfikowany klient albo inna wersja generatora),
3. (z pierwszymi mechanikami) waliduje intencje tą samą funkcją rdzenia co klienci, zanim trafią do tury,
4. (później) robi snapshoty stanu dla dołączających i autorytatywnie zgłasza wynik meczu.

Cień jest opcją pokoju; bez niego działa dotychczasowa reguła „pierwszy hash wygrywa”.

## Konsekwencje

- Koszt: generowanie mapy na serwerze (ok. 3 s CPU, kilkadziesiąt MB RAM na pokój) – robione przy tworzeniu pokoju, przed startem gry. Na t4g.micro starczy na kilka równoległych pokoi.
- **Ograniczenie:** w lockstepie każdy klient ma pełny stan gry, więc cheatów informacyjnych (maphack, podgląd pod mgłę wojny) nie da się zablokować – cień wykrywa tylko manipulację stanem i intencjami. Ochrona przed maphackiem wymagałaby serwera autorytatywnego, który wysyła graczom tylko widoczny stan – inna architektura, świadomie poza zakresem.
