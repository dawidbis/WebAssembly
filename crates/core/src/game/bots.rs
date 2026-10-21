//! Proste boty – gracze sterowani przez komputer, liczeni w rdzeniu u każdego gracza tak samo (lockstep).
//!
//! Bot to zwykły uczestnik gry (`Game::bots`): co `BOT_PERIOD` tur (każdy w innej turze) patrzy na stan
//! swojego państwa i wykonuje **te same intencje co gracz** (`Game::execute` – z tą samą walidacją), więc
//! nie ma żadnych osobnych zasad. Decyzje wynikają tylko ze stanu gry – bez losowości i bez zegara.
//!
//! - **Start:** na koniec wyboru jak start losowy, ale najchętniej prowincja nad morzem (port i handel).
//! - **Suwak wojsko/robotnicy:** więcej wojska, gdy jest pustkowie do zajęcia albo wojna, mniej, gdy brakuje
//!   robotników do obsady budynków.
//! - **Ekspansja:** fronty na sąsiednie prowincje z ziemią niczyją (bez desantów), w granicach centrum
//!   dowodzenia; wybór według wartości (surowiec, wybrzeże) do siły tubylców.
//! - **Budowy:** po jednej naraz – złoto, miasta przy limicie populacji, port, fabryka zaopatrzenia, którego
//!   najbardziej brakuje ludności (prowiant, podstawowe, materiały, paliwo, luksusowe), wydobycie przydatnych
//!   surowców, infrastruktura, gdy brakuje punktów, centrum dowodzenia przy nadmiarze złota.
//! - **Rynek:** import surowców, których brakuje fabrykom, eksport reszty.
//! - **Wojna:** bot nigdy nie zaczyna. Kto wypowie mu wojnę w prowincji (także atakiem na jego ziemię), ten ma
//!   u niego uraz (`Game::grudges`) – bot kontratakuje w tych prowincjach, a gdy jest dużo silniejszy, także
//!   w sąsiedniej prowincji napastnika. Po `GRUDGE_TICKS` spokoju odwołuje swoje wojny i fronty z nim.
//!
//! Stałe do strojenia są niżej – to zasady gry (u wszystkich takie same), więc bez suwaków w panelu debugu.

use super::{Game, Phase, Rng, START_SALT};
use crate::{
    buildings::{BuildBlock, Building, STAFF_PER_LEVEL},
    economy::{BASIC_PEOPLE_PER_UNIT, LUXURY_PEOPLE_PER_UNIT, MILLI, PROVISIONS_PEOPLE_PER_UNIT},
    factories::{COMMAND_LEVEL, Input, NO_SHORTAGE, OUTPUT_PER_LEVEL_PER_DAY, RECIPES},
    goods::{GOLD, RESOURCE_COUNT, SUPPLY_COUNT, Supply, TICKS_PER_DAY},
    market,
    protocol::{Intent, PlayerId, SELECTION_TICKS},
    war::{self, MAX_FRONTS, MIN_ATTACK_TROOPS},
};

/// Wybór startu: pierwszy wybór po tylu turach od początku wyboru (+ przesunięcie bota), tyle kroków (od–do)
/// od celu, krok na sąsiednią prowincję co `HOP_TICKS`, bez kroków na `SETTLE_TICKS` przed końcem.
pub const FIRST_PICK_TICKS: u32 = 20;
pub const WALK_STEPS: (u8, u8) = (3, 5);
pub const HOP_TICKS: u32 = 25;
pub const SETTLE_TICKS: u32 = 30;
/// Ekspansja: waga zwartości (za każdy % sąsiadów prowincji, którzy są już nasi) i przesunięcie odległości od
/// stolicy (kafle) – im mniejsze, tym mocniej bot woli prowincje blisko stolicy.
pub const COMPACT_WEIGHT: u64 = 4;
pub const DIST_OFFSET: u64 = 40;
/// Ziarno przeskoków przy wyborze startu.
const PICK_SALT: u64 = 0x424F_5453_0002;

/// Bot decyduje co tyle tur (2 dni gry); boty po kolei w różnych turach (`BOT_STAGGER`).
pub const BOT_PERIOD: u32 = 20;
const BOT_STAGGER: u32 = 7;
/// Udział wojska (%): przy wojnie, przy ekspansji i w spokoju (gdy nie ma już pustkowia w zasięgu).
pub const WAR_SHARE: u8 = 60;
pub const EXPAND_SHARE: u8 = 30;
pub const PEACE_SHARE: u8 = 20;
/// Przy pokryciu obsady poniżej tylu % (części na milion) bot zmniejsza udział wojska o `STAFF_SHARE_CUT`.
pub const STAFF_LOW_PPM: u64 = 800_000;
pub const STAFF_SHARE_CUT: u8 = 15;
/// Nowy front: najmniej tylu żołnierzy w kraju, tyle % z nich idzie na front, a siła wysłana ma być co najmniej
/// tyle % armii tubylczej prowincji.
pub const FRONT_MIN_TROOPS: u64 = 1_000;
pub const FRONT_PERCENT: u8 = 70;
pub const FRONT_NATIVES_PCT: u64 = 80;
/// Na front idzie tyle % armii tubylczej prowincji (najmniej `FRONT_MIN_TROOPS`, najwyżej `FRONT_PERCENT`% wojska).
pub const FRONT_SEND_PCT: u64 = 250;
/// Ilu najlepszych kandydatów do ataku bot sprawdza (`Game::check_attack`) w jednej decyzji.
const FRONT_CANDIDATES: usize = 4;
/// Uraz wygasa po tylu turach bez walki z napastnikiem (2 miesiące gry) – wtedy bot zawiera pokój.
pub const GRUDGE_TICKS: u32 = 600;
/// Bot atakuje ziemię napastnika poza spornymi prowincjami, gdy jego armia ma ponad tyle % armii napastnika.
pub const STRIKE_ARMY_PCT: u64 = 150;
/// Złoto (surowe) zostawiane w skarbcu po każdej budowie – na import.
pub const GOLD_RESERVE: u64 = 20_000;
/// Miasto, gdy populacja przekroczy tyle % limitu, a prowiantu starcza (pokrycie w częściach na milion).
pub const CITY_FILL_PCT: u64 = 85;
pub const CITY_FOOD_PPM: u64 = 900_000;
/// Najwięcej budów rozpoczętych w jednej decyzji.
pub const BUILDS_PER_TURN: usize = 2;
/// Kolejny poziom fabryki: o tyle mniej ważny za każdy poziom, który już ma.
pub const UPGRADE_PENALTY: u64 = 60;
/// Najwyższy poziom wydobycia, który bot buduje (złoto – bez limitu poza `MAX_LEVEL`).
pub const EXTRACTION_LEVEL: u8 = 3;
/// Obsada: nowy budynek z robotnikami tylko, gdy obsada po nim nie przekroczy tylu % robotników.
pub const STAFF_LIMIT_PCT: u64 = 110;
/// Zapas zaopatrzenia (dni zużycia ludności), poniżej którego bot chce fabryki; materiały i paliwo –
/// zapas w jednostkach (zużywają je budowy i ataki).
pub const SUPPLY_DAYS: u64 = 60;
pub const MATERIALS_STOCK: u64 = 80;
pub const FUEL_STOCK: u64 = 60;
/// Waga braku prowiantu (%), luksusowe dopiero przy takim pokryciu podstawowego (części na milion), a brak
/// zaopatrzenia, na którym stoją nasze fabryki, liczy się o tyle więcej (promile – głód i tak wygrywa).
pub const FOOD_WEIGHT_PCT: u64 = 150;
pub const LUXURY_BASIC_PPM: u64 = 990_000;
/// Materiały poniżej tylu jednostek blokują budowy – fabryka materiałów przed resztą. Produkcja materiałów
/// i paliwa liczy się do zapasu za tyle dni.
pub const MATERIALS_LOW: u64 = 30;
pub const MADE_DAYS: u64 = 20;
const BLOCKING_SHORTAGE: u64 = 1500;
/// Centrum dowodzenia: kolejny poziom, gdy złota jest co najmniej tyle razy więcej niż kosztuje.
pub const COMMAND_GOLD_TIMES: u64 = 3;
/// Rynek (dni zużycia fabryk): import, gdy zapasu jest mniej, eksport nadwyżki ponad drugi próg; reszta
/// surowców (których fabryki nie zużywają) – eksport, gdy zapas przekroczy `SPARE_STOCK` jednostek.
pub const IMPORT_DAYS: u64 = 10;
pub const EXPORT_DAYS: u64 = 40;
pub const SPARE_STOCK: u64 = 2;
pub const EXPORT_SURPLUS_PCT: i8 = 30;

/// Zaopatrzenie, którego chce ludność i budowy – w tej kolejności przy remisie.
const CIVIL: [Supply; 5] =
    [Supply::Provisions, Supply::BasicCivil, Supply::EngineeringMaterials, Supply::Fuel, Supply::LuxuryCivil];

/// Ziarno startu bota dołączającego w trakcie gry.
const LATE_START_SALT: u64 = 0x424F_5453_0001;

impl Game {
    /// Gracze sterowani przez komputer.
    pub fn bots(&self) -> &std::collections::BTreeSet<PlayerId> {
        &self.bots
    }

    /// `Intent::SetBot` od serwera. Nowy bot przed końcem wyboru – uczestnik ze startem na koniec wyboru,
    /// w grze właściwej – od razu start (jak losowy, najchętniej nad morzem).
    pub(super) fn set_bot(&mut self, player: PlayerId, bot: bool) {
        if !bot {
            self.bots.remove(&player);
            return;
        }
        self.bots.insert(player);
        match self.phase {
            Phase::Waiting => {}
            Phase::Selection { .. } => {
                if let Err(k) = self.players.binary_search(&player) {
                    self.players.insert(k, player);
                }
            }
            Phase::Playing => {
                if self.owners.nation_of(player).is_some() {
                    return;
                }
                let mut rng =
                    Rng(self.config.map.seed as u64 ^ ((self.tick as u64) << 32) ^ START_SALT ^ LATE_START_SALT);
                let Some(province) = self.random_start(&mut rng, true) else { return };
                if let Err(k) = self.players.binary_search(&player) {
                    self.players.insert(k, player);
                }
                if self.take_start(player, province) {
                    let levels = &mut self.buildings[province as usize - 1].levels;
                    for b in [Building::CommandCenter, Building::City] {
                        levels[b as usize] = levels[b as usize].max(1);
                    }
                    self.buildings_revision = self.buildings_revision.wrapping_add(1);
                }
            }
        }
    }

    /// Tura botów (gra właściwa): każdy bot decyduje w swojej turze co `BOT_PERIOD`.
    pub(super) fn bots_tick(&mut self) {
        let bots: Vec<PlayerId> = self.bots.iter().copied().collect();
        for (k, player) in bots.into_iter().enumerate() {
            let k = k as u32;
            match self.phase {
                Phase::Selection { end } => self.bot_select(player, k, end),
                Phase::Playing if (self.tick + k * BOT_STAGGER).is_multiple_of(BOT_PERIOD) => self.bot_turn(player),
                _ => {}
            }
        }
    }

    /// Wybór startu jak człowiek: bot losuje cel jak start losowy (najchętniej nad morzem, z dala od innych), po
    /// chwili staje kilka prowincji od niego (`WALK_STEPS` – widać go na mapie) i co `HOP_TICKS` przechodzi na
    /// sąsiednią prowincję bliżej celu. Gdy drogę albo cel zajmie ktoś inny, zostaje tam, gdzie jest. Cel bota
    /// to `Game::bot_targets` (w hashu, tylko w czasie wyboru).
    fn bot_select(&mut self, player: PlayerId, k: u32, end: u32) {
        let left = end - self.tick;
        let wait = FIRST_PICK_TICKS + k * BOT_STAGGER % HOP_TICKS;
        let mut rng =
            Rng(self.config.map.seed as u64 ^ ((self.tick as u64) << 32) ^ ((player as u64) << 16) ^ PICK_SALT);
        match self.starts.get(&player).copied() {
            // Przy starcie losowym (wybór bez czasu) start daje `finish_selection`.
            None if left + wait <= SELECTION_TICKS && left > SETTLE_TICKS => {
                let Some(target) = self.random_start(&mut rng, true) else { return };
                let steps = WALK_STEPS.0 + rng.below((WALK_STEPS.1 - WALK_STEPS.0 + 1) as usize) as u8;
                let dist = self.walk_distances(target, steps, 0);
                // Najdalsze osiągalne prowincje (do `steps` kroków) – stamtąd bot rusza do celu.
                let far = dist.iter().filter(|&&d| d != u8::MAX).max().copied().unwrap_or(0);
                let from: Vec<u16> = (1..=dist.len() as u16).filter(|&p| dist[p as usize - 1] == far).collect();
                let first = if from.is_empty() { target } else { from[rng.below(from.len())] };
                self.bot_targets.insert(player, target);
                self.choose_start(player, first);
            }
            Some(at) if left > SETTLE_TICKS && (self.tick + k * BOT_STAGGER).is_multiple_of(HOP_TICKS) => {
                let Some(&target) = self.bot_targets.get(&player) else { return };
                if at == target {
                    return;
                }
                let dist = self.walk_distances(target, WALK_STEPS.1 + 1, at);
                let here = dist[at as usize - 1];
                let next: Vec<u16> = self
                    .neighbours(at)
                    .iter()
                    .map(|l| l.province)
                    .filter(|&p| self.free_start(p) && dist[p as usize - 1] < here)
                    .collect();
                if next.is_empty() || !self.free_start(target) {
                    // Droga zajęta – bot zostaje.
                    self.bot_targets.insert(player, at);
                } else {
                    self.choose_start(player, next[rng.below(next.len())]);
                }
            }
            _ => {}
        }
    }

    /// Odległości (w krokach po sąsiednich prowincjach wolnych na start, a także przez `through` – prowincję bota)
    /// od `from`, najwyżej `limit` kroków; `u8::MAX` – dalej albo nieosiągalna.
    fn walk_distances(&self, from: u16, limit: u8, through: u16) -> Vec<u8> {
        let mut dist = vec![u8::MAX; self.map.provinces.len()];
        dist[from as usize - 1] = 0;
        let mut ring = vec![from];
        for d in 1..=limit {
            let mut next = Vec::new();
            for &p in &ring {
                for q in self.neighbours(p).iter().map(|l| l.province) {
                    if dist[q as usize - 1] == u8::MAX && (q == through || self.free_start(q)) {
                        dist[q as usize - 1] = d;
                        next.push(q);
                    }
                }
            }
            ring = next;
        }
        dist
    }

    fn bot_turn(&mut self, player: PlayerId) {
        let Some(n) = self.owners.nation_of(player) else { return };
        if self.annexed.contains_key(&n) || self.owners.area(n) == 0 {
            return;
        }
        self.bot_grudges(player, n);
        let at_war = self.grudges.keys().any(|&(b, _)| b == n);
        let targets = self.expansion_targets(n);
        self.bot_share(player, n, at_war, !targets.is_empty());
        if at_war {
            self.bot_strike(player, n);
        }
        self.bot_expand(player, n, &targets);
        self.bot_build(player, n);
        self.bot_retool(player, n);
        self.bot_trade(player, n);
    }

    /// Urazy: odświeża tych, którzy walczą z botem; po `GRUDGE_TICKS` spokoju – pokój z napastnikiem
    /// (odwołane wojny bota i jego fronty na ziemię napastnika bez kafli niczyich).
    fn bot_grudges(&mut self, player: PlayerId, n: u8) {
        let attackers: Vec<u8> = self.hostility.iter().filter(|h| h.2 == n && h.1 != n).map(|h| h.1).collect();
        for m in attackers {
            self.grudges.insert((n, m), self.tick);
        }
        let expired: Vec<u8> = self
            .grudges
            .iter()
            .filter(|&(&(b, _), &last)| b == n && self.tick >= last + GRUDGE_TICKS)
            .map(|(&(_, m), _)| m)
            .collect();
        for m in expired {
            self.grudges.remove(&(n, m));
            let Some(&other) = self.owners.nations().get(m as usize - 1) else { continue };
            let ours: Vec<u16> = self.hostility.iter().filter(|h| h.1 == n && h.2 == m).map(|h| h.0).collect();
            for &province in &ours {
                self.execute(player, &Intent::SetStance { province, player: other, war: false });
            }
            let fronts: Vec<u16> = self
                .attacks
                .iter()
                .filter(|a| a.nation == n && !a.defense && !a.reinforce)
                .filter(|a| a.provinces.iter().all(|&p| !self.has_neutral(p)))
                .map(|a| a.provinces[0])
                .collect();
            for province in fronts {
                self.execute(player, &Intent::CancelAttack { province });
            }
        }
    }

    /// Suwak wojsko/robotnicy według sytuacji (zmiana tylko, gdy cel jest inny).
    fn bot_share(&mut self, player: PlayerId, n: u8, at_war: bool, expanding: bool) {
        let mut share = if at_war {
            WAR_SHARE
        } else if expanding {
            EXPAND_SHARE
        } else {
            PEACE_SHARE
        };
        let rates = self.rates();
        if !at_war && rates[n as usize - 1].staff_ppm < STAFF_LOW_PPM {
            share = share.saturating_sub(STAFF_SHARE_CUT).max(PEACE_SHARE / 2);
        }
        if self.economy[n as usize - 1].target_share != share {
            self.execute(player, &Intent::SetTroopShare { percent: share });
        }
    }

    /// Fronty bota (bez armii obrony i posiłków desantem).
    fn bot_fronts(&self, n: u8) -> usize {
        self.attacks.iter().filter(|a| a.nation == n && !a.defense && !a.reinforce).count()
    }

    fn has_neutral(&self, province: u16) -> bool {
        let held: u32 = self.owners.shares(province).iter().map(|s| s.1).sum();
        held < self.map.provinces[province as usize - 1].area
    }

    /// Wolne miejsce na nowy front (limit centrum dowodzenia).
    fn front_slot(&self, n: u8) -> bool {
        self.bot_fronts(n) < MAX_FRONTS[self.command_level(n).min(6) as usize]
    }

    /// Kontratak na napastników: sporne prowincje (wojna z napastnikiem, ma w nich kafle), a przy dużej
    /// przewadze także jego prowincja obok. Bez wolnego miejsca – odwołany front na pustkowie.
    fn bot_strike(&mut self, player: PlayerId, n: u8) {
        let enemies: Vec<u8> = self.grudges.keys().filter(|&&(b, _)| b == n).map(|&(_, m)| m).collect();
        for m in enemies {
            if self.economy[n as usize - 1].troops < FRONT_MIN_TROOPS * MILLI {
                return;
            }
            let fighting = |g: &Game, p: u16| g.attacks.iter().any(|a| a.nation == n && a.provinces.contains(&p));
            let mut targets: Vec<u16> = self
                .hostility
                .iter()
                .filter(|h| (h.1 == m && h.2 == n) || (h.1 == n && h.2 == m))
                .map(|h| h.0)
                .filter(|&p| self.owners.shares(p).iter().any(|s| s.0 == m))
                .collect();
            targets.dedup();
            let (army, enemy) = (self.economy[n as usize - 1].army(), self.economy[m as usize - 1].army());
            if army * 100 > enemy * STRIKE_ARMY_PCT {
                // Prowincja napastnika obok naszej ziemi (pierwsza po numerze).
                let ours = self.bot_provinces(n);
                let next = ours.iter().flat_map(|&p| self.neighbours(p).iter().map(|l| l.province)).find(|&q| {
                    self.owners.province_owner(q) == Some(m) && !self.owners.shares(q).iter().any(|s| s.0 == n)
                });
                targets.extend(next);
            }
            for p in targets {
                if fighting(self, p) {
                    continue;
                }
                if !self.front_slot(n) {
                    // Miejsce na kontratak kosztem frontu na pustkowie.
                    let spare = self
                        .attacks
                        .iter()
                        .find(|a| {
                            a.nation == n
                                && !a.defense
                                && !a.reinforce
                                && a.provinces.iter().all(|&q| self.has_neutral(q))
                        })
                        .map(|a| a.provinces[0]);
                    match spare {
                        Some(q) => self.execute(player, &Intent::CancelAttack { province: q }),
                        None => return,
                    }
                }
                if matches!(self.check_attack(player, &[p]), Ok(None)) {
                    self.execute(player, &Intent::Attack { provinces: vec![p], percent: FRONT_PERCENT });
                }
            }
        }
    }

    /// Prowincje, w których państwo ma kafle (rosnąco).
    fn bot_provinces(&self, n: u8) -> Vec<u16> {
        (1..=self.map.provinces.len() as u16).filter(|&p| self.owners.shares(p).iter().any(|s| s.0 == n)).collect()
    }

    /// Sąsiednie prowincje z ziemią niczyją, od najlepszej: wartość (surowiec, wybrzeże) przez siłę tubylców.
    /// Bez prowincji, na które już idzie nasz front.
    fn expansion_targets(&self, n: u8) -> Vec<(u64, u16)> {
        let ours = self.bot_provinces(n);
        let owned = self.owned_resources(n);
        let provinces = &self.map.provinces;
        // Środek stolicy – terytorium rośnie wokół niej mniej więcej po równo.
        let (cx, cy) = self.capital(n).map_or((0, 0), |c| {
            let c = &provinces[c as usize - 1];
            (c.center_x as i64, c.center_y as i64)
        });
        let mut seen = vec![false; provinces.len()];
        let mut out = Vec::new();
        for &p in &ours {
            for q in self.neighbours(p).iter().map(|l| l.province).chain([p]) {
                let k = q as usize - 1;
                if seen[k]
                    || !self.has_neutral(q)
                    || self.attacks.iter().any(|a| a.nation == n && a.provinces.contains(&q))
                {
                    continue;
                }
                seen[k] = true;
                let pr = &provinces[k];
                let mut value = 200 + resource_value(pr.resource) * if owned[pr.resource as usize] { 1 } else { 2 } / 2;
                if pr.coastal {
                    value += 40;
                }
                // Cudzy udział – możliwy spór, mniej chętnie.
                if self.owners.shares(q).iter().any(|s| s.0 != n) {
                    value /= 2;
                }
                // Zwartość: ile sąsiadów prowincji już do nas należy (wypełnia wcięcia, bez długich odnóg).
                let links = self.neighbours(q);
                let ours_pct =
                    links.iter().filter(|l| self.owners.province_owner(l.province) == Some(n)).count() as u64 * 100
                        / links.len().max(1) as u64;
                let dist = ((pr.center_x as i64 - cx).pow(2) + (pr.center_y as i64 - cy).pow(2)).isqrt() as u64;
                // Gęstość tubylców (% zwykłej) – gęsto zaludnione później.
                let density = self.natives[k] * 100 / (war::natives(pr.area, 1).max(1));
                let score =
                    value * (100 + COMPACT_WEIGHT * ours_pct) * 10_000 / ((dist + DIST_OFFSET) * (100 + density));
                out.push((score, q));
            }
        }
        out.sort_unstable_by(|a, b| b.cmp(a));
        out
    }

    /// Ekspansja: nowy front na najlepszą prowincję z pustkowiem, gdy jest miejsce i wojsko.
    fn bot_expand(&mut self, player: PlayerId, n: u8, targets: &[(u64, u16)]) {
        if !self.front_slot(n) {
            return;
        }
        let troops = self.economy[n as usize - 1].troops;
        if troops < FRONT_MIN_TROOPS * MILLI {
            return;
        }
        let most = troops * FRONT_PERCENT as u64 / 100;
        for &(_, p) in targets.iter().take(FRONT_CANDIDATES) {
            let natives = self.natives[p as usize - 1];
            if most * 100 < natives * FRONT_NATIVES_PCT || most < MIN_ATTACK_TROOPS * MILLI {
                continue;
            }
            // Tyle, ile trzeba na tubylców (fronty spalają paliwo według liczby wojska), najwyżej `FRONT_PERCENT`%.
            let sent = (natives * FRONT_SEND_PCT / 100).max(FRONT_MIN_TROOPS * MILLI).min(most);
            let percent = (sent * 100).div_ceil(troops).clamp(1, FRONT_PERCENT as u64) as u8;
            // Bez desantów – tylko lądem (także przeprawą).
            if matches!(self.check_attack(player, &[p]), Ok(None)) {
                self.execute(player, &Intent::Attack { provinces: vec![p], percent });
                return;
            }
        }
    }

    /// Surowce, które państwo już wydobywa (kontroluje wydobycie w prowincji z nimi).
    fn owned_resources(&self, n: u8) -> [bool; RESOURCE_COUNT] {
        let mut owned = [false; RESOURCE_COUNT];
        for (k, p) in self.map.provinces.iter().enumerate() {
            if self.controller(k as u16 + 1, Building::Extraction) == Some(n) {
                owned[p.resource as usize] = true;
            }
        }
        owned
    }

    /// Brak zaopatrzenia w promilach zapasu docelowego (0 – wystarczy) plus dzienny niedobór produkcji w promilach
    /// zużycia ludności: do 1000 – za mały zapas, do 2000 – do tego produkcja nie nadąża.
    fn supply_shortage(&self, n: u8, s: Supply) -> u64 {
        let e = &self.economy[n as usize - 1];
        let people = e.population() / MILLI;
        let per_day = match s {
            Supply::Provisions => people * MILLI / PROVISIONS_PEOPLE_PER_UNIT,
            Supply::BasicCivil => people * MILLI / BASIC_PEOPLE_PER_UNIT,
            Supply::LuxuryCivil => people * MILLI / LUXURY_PEOPLE_PER_UNIT,
            // Paliwo jako składnik naszych fabryk (fronty spalają każdy zapas – ich nie liczymy).
            Supply::Fuel => self.factory_fuel(n),
            _ => 0,
        };
        let target = match s {
            Supply::EngineeringMaterials => MATERIALS_STOCK * MILLI,
            Supply::Fuel => FUEL_STOCK * MILLI,
            _ => per_day * SUPPLY_DAYS,
        }
        .max(MILLI);
        let made = e.supply_output[s as usize] * TICKS_PER_DAY;
        // Materiały (zużywane od razu przy budowie) – zapas razem z produkcją z `MADE_DAYS` dni.
        let stock = e.supplies[s as usize] + if per_day == 0 { made * MADE_DAYS } else { 0 };
        let short = target.saturating_sub(stock) * 1000 / target;
        short + (per_day.saturating_sub(made) * 1000).checked_div(per_day).unwrap_or(0)
    }

    /// Paliwo na dzień (tysięczne) dla pełnej produkcji naszych fabryk, których receptura może je brać.
    fn factory_fuel(&self, n: u8) -> u64 {
        let mut need = 0;
        for p in 1..=self.map.provinces.len() as u16 {
            let b = self.buildings(p);
            let level = b.level(Building::Factory) as u64;
            if level == 0 || self.controller(p, Building::Factory) != Some(n) {
                continue;
            }
            for options in RECIPES[b.product as usize] {
                if let Some(a) = options.iter().find(|a| a.input == Input::Supply(Supply::Fuel)) {
                    need += level * OUTPUT_PER_LEVEL_PER_DAY * a.amount / MILLI;
                }
            }
        }
        need
    }

    /// Czy państwo zdobędzie składniki receptury: każdy składnik ma wejście z zapasu, z wydobycia albo z rynku
    /// (przy porcie – od handlarzy).
    fn recipe_feasible(&self, n: u8, s: Supply, port: bool) -> bool {
        let e = &self.economy[n as usize - 1];
        let owned = self.owned_resources(n);
        RECIPES[s as usize].iter().all(|options| {
            options.iter().filter(|a| a.universities == 0).any(|a| match a.input {
                Input::Resource(r) => port || owned[r as usize] || e.stock[r as usize] > 0,
                Input::Supply(x) => e.supplies[x as usize] > 0,
            })
        })
    }

    /// Brakujące zaopatrzenie, które da się zrobić (odblokowane, składniki osiągalne) – od największego braku.
    /// Prowiant waży `FOOD_WEIGHT_PCT`%, luksusowe dopiero przy pełnym podstawowym, a zaopatrzenie, na którym
    /// stoją nasze fabryki (składnik – np. paliwo w podstawowym), idzie przed resztę.
    fn wanted_supplies(&self, n: u8, port: bool) -> Vec<(u64, Supply)> {
        let command = self.command_level(n);
        let e = &self.economy[n as usize - 1];
        let mut blocking = [false; SUPPLY_COUNT];
        for p in 1..=self.map.provinces.len() as u16 {
            let b = self.buildings(p);
            if b.level(Building::Factory) == 0
                || b.short == NO_SHORTAGE
                || self.controller(p, Building::Factory) != Some(n)
            {
                continue;
            }
            for a in RECIPES[b.product as usize].get(b.short as usize).copied().unwrap_or_default() {
                if let Input::Supply(x) = a.input {
                    blocking[x as usize] = true;
                }
            }
        }
        // Bez materiałów stoją budowy (miasta, infrastruktura, kolejne poziomy).
        let materials = Supply::EngineeringMaterials as usize;
        blocking[materials] |= e.supplies[materials] < MATERIALS_LOW * MILLI;
        // Luksusowe dopiero, gdy prowiantu i podstawowego nie brakuje.
        let luxury = e.basic_ppm >= LUXURY_BASIC_PPM
            && self.supply_shortage(n, Supply::Provisions) == 0
            && self.supply_shortage(n, Supply::BasicCivil) == 0;
        let mut out: Vec<(u64, Supply)> = CIVIL
            .iter()
            .filter(|&&s| command >= COMMAND_LEVEL[s as usize] && self.recipe_feasible(n, s, port))
            .filter(|&&s| s != Supply::LuxuryCivil || luxury)
            .map(|&s| {
                let short = self.supply_shortage(n, s);
                let short = if s == Supply::Provisions { short * FOOD_WEIGHT_PCT / 100 } else { short };
                (if blocking[s as usize] && short > 0 { short + BLOCKING_SHORTAGE } else { short }, s)
            })
            .filter(|&(short, _)| short > 0)
            .collect();
        // Stabilnie – przy remisie kolejność `CIVIL`.
        out.sort_by_key(|a| std::cmp::Reverse(a.0));
        out
    }

    /// Budowy (najwyżej `BUILDS_PER_TURN` na decyzję): kandydaci od najważniejszego, każdy możliwy z zapasem
    /// złota. Na najważniejszą budowę bez złota bot czeka (oszczędza); brak materiałów – próbuje dalej; brak
    /// punktów infrastruktury – infrastruktura w tej prowincji.
    fn bot_build(&mut self, player: PlayerId, n: u8) {
        let provinces: Vec<u16> =
            self.bot_provinces(n).into_iter().filter(|&p| self.owners.province_owner(p) == Some(n)).collect();
        let rates = self.rates();
        let r = &rates[n as usize - 1];
        let e = &self.economy[n as usize - 1];
        let workers = e.workers / MILLI;
        let staffed = |b: Building| r.staff + STAFF_PER_LEVEL[b as usize] <= workers * STAFF_LIMIT_PCT / 100;
        let port = provinces.iter().any(|&p| self.buildings(p).level(Building::Port) > 0);
        let supplies = self.wanted_supplies(n, port);
        // Składnik brakującego zaopatrzenia – wydobycie ważniejsze o część braku.
        let needed = |resource: u8| {
            supplies
                .iter()
                .filter(|(_, s)| {
                    RECIPES[*s as usize].iter().any(|o| o.iter().any(|a| a.input == Input::Resource(resource)))
                })
                .map(|&(short, _)| short / 5)
                .max()
                .unwrap_or(0)
        };
        let crowded = e.population() * 100 >= r.cap * MILLI * CITY_FILL_PCT && e.food_ppm >= CITY_FOOD_PPM;
        let mut wanted: Vec<(u64, u16, Building, Option<Supply>)> = Vec::new();
        for &p in &provinces {
            let b = self.buildings(p);
            if b.construction.is_some() {
                continue;
            }
            let pr = &self.map.provinces[p as usize - 1];
            let level = b.level(Building::Extraction);
            if pr.resource == GOLD {
                wanted.push((1000, p, Building::Extraction, None));
            } else if level < EXTRACTION_LEVEL && staffed(Building::Extraction) {
                let score = 250 + resource_value(pr.resource) + needed(pr.resource);
                wanted.push((score.saturating_sub(level as u64 * 80), p, Building::Extraction, None));
            }
            if crowded && b.level(Building::City) > 0 {
                wanted.push((600 + b.level(Building::City) as u64, p, Building::City, None));
            }
            if !port && pr.coastal && staffed(Building::Port) {
                wanted.push((700, p, Building::Port, None));
            }
        }
        // Nowe miasto – w największej prowincji.
        if crowded && let Some(&p) = provinces.iter().max_by_key(|&&p| (self.map.provinces[p as usize - 1].area, p)) {
            wanted.push((550, p, Building::City, None));
        }
        if staffed(Building::Factory) {
            for &(short, s) in supplies.iter().take(2) {
                let score = 300 + short / 4;
                // Nowa fabryka (poziom 1 – tani i bez materiałów), a kolejny poziom tej z tym produktem tym mniej
                // chętnie, im jest wyższy (koszt rośnie ×1,5 za poziom).
                for &p in &provinces {
                    let b = self.buildings(p);
                    let level = b.level(Building::Factory) as u64;
                    if b.construction.is_none() && level > 0 && b.product == s {
                        wanted.push((score.saturating_sub(level * UPGRADE_PENALTY), p, Building::Factory, None));
                    }
                    if b.construction.is_none() && b.level(Building::Factory) == 0 {
                        wanted.push((score, p, Building::Factory, Some(s)));
                    }
                }
            }
        }
        if let Some(c) = self.capital(n)
            && let Ok((gold, _)) = self.check_build(player, c, Building::CommandCenter)
            && e.gold >= gold * COMMAND_GOLD_TIMES
        {
            wanted.push((200, c, Building::CommandCenter, None));
        }
        wanted.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut built = 0;
        for (_, p, building, product) in wanted {
            if built >= BUILDS_PER_TURN {
                return;
            }
            let gold = self.economy[n as usize - 1].gold;
            match self.check_build(player, p, building) {
                Ok((cost, _)) if gold >= cost + GOLD_RESERVE => {
                    if let Some(s) = product {
                        self.execute(player, &Intent::SetFactory { province: p, supply: s });
                    }
                    self.execute(player, &Intent::Build { province: p, building });
                    built += 1;
                }
                Err(BuildBlock::NoPoints) => {
                    if let Ok((cost, _)) = self.check_build(player, p, Building::Infrastructure)
                        && gold >= cost + GOLD_RESERVE
                    {
                        self.execute(player, &Intent::Build { province: p, building: Building::Infrastructure });
                        built += 1;
                    }
                }
                // Najważniejsza budowa czeka na złoto – bez wydawania na mniej ważne.
                Ok(_) | Err(BuildBlock::NoGold) => return,
                Err(_) => {}
            }
        }
    }

    /// Przezbrojenie fabryki, która stoi bez składnika, którego nie da się zdobyć – na inne potrzebne zaopatrzenie.
    fn bot_retool(&mut self, player: PlayerId, n: u8) {
        let port = (1..=self.map.provinces.len() as u16)
            .any(|p| self.buildings(p).level(Building::Port) > 0 && self.controller(p, Building::Port) == Some(n));
        for p in 1..=self.map.provinces.len() as u16 {
            let b = self.buildings(p);
            if b.level(Building::Factory) == 0
                || self.controller(p, Building::Factory) != Some(n)
                || b.retool > 0
                || b.output > 0
                || b.short == NO_SHORTAGE
                || self.recipe_feasible(n, b.product, port)
            {
                continue;
            }
            if let Some((_, s)) = self.wanted_supplies(n, port).first().copied()
                && s != b.product
            {
                self.execute(player, &Intent::SetFactory { province: p, supply: s });
                return;
            }
        }
    }

    /// Rynek: suwaki surowców według potrzeb fabryk (import braków, eksport nadwyżek i surowców bez użytku).
    fn bot_trade(&mut self, player: PlayerId, n: u8) {
        let rates = self.rates();
        let needs = self.needs(n, &rates[n as usize - 1]);
        if needs.capacity == 0 {
            return;
        }
        let e = &self.economy[n as usize - 1];
        let rich = e.gold > GOLD_RESERVE;
        let mut settings = Vec::new();
        for r in (0..RESOURCE_COUNT).filter(|&r| market::tradable(r)) {
            let (need, stock) = (needs.need[r], e.stock[r]);
            let percent: i8 = if need > 0 {
                if stock < need * IMPORT_DAYS && rich {
                    -100
                } else if stock > need * EXPORT_DAYS {
                    EXPORT_SURPLUS_PCT
                } else {
                    0
                }
            } else if stock > SPARE_STOCK * MILLI {
                100
            } else {
                0
            };
            if e.trade.settings[r] != percent {
                settings.push((r as u8, percent));
            }
        }
        for (resource, percent) in settings {
            self.execute(player, &Intent::SetTradeResource { resource, percent });
        }
    }
}

/// Wartość surowca dla bota: składnik zaopatrzenia ludności i budów (z wagą jego ważności), złoto najwyżej.
fn resource_value(resource: u8) -> u64 {
    if resource == GOLD {
        return 400;
    }
    const WEIGHT: [u64; 5] = [120, 80, 80, 50, 30];
    CIVIL
        .iter()
        .zip(WEIGHT)
        .filter(|(s, _)| {
            RECIPES[**s as usize].iter().any(|options| options.iter().any(|a| a.input == Input::Resource(resource)))
        })
        .map(|(_, w)| w)
        .sum::<u64>()
        .min(200)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::tests::config,
        protocol::{SERVER, StampedIntent, Turn},
    };

    fn run(game: &mut Game, ticks: u32) {
        for _ in 0..ticks {
            let tick = game.tick();
            game.apply_turn(&Turn { tick, intents: Vec::new() });
        }
    }

    fn with_bots(humans: &[PlayerId], bots: &[PlayerId]) -> Game {
        let mut game = Game::new(config());
        let begin = Intent::BeginSelection { players: humans.to_vec(), random: true, bots: bots.to_vec() };
        let mut t = Turn { tick: 0, intents: Vec::new() };
        t.intents.push(StampedIntent { player: SERVER, intent: begin });
        game.apply_turn(&t);
        run(&mut game, 1);
        game
    }

    #[test]
    fn bots_start_by_the_sea_and_expand_build_and_trade() {
        let mut game = with_bots(&[0], &[10, 11]);
        assert_eq!(game.phase(), Phase::Playing);
        for bot in [10, 11] {
            let start = game.starts()[&bot];
            assert!(game.map().provinces[start as usize - 1].coastal, "bot {bot} startuje nad morzem");
        }
        let n = game.owners().nation_of(10).unwrap();
        let area = game.owners().area(n);
        // Pół roku gry.
        run(&mut game, 6 * crate::economy::TAX_TICKS);
        assert!(game.owners().area(n) > area, "bot zajmuje pustkowie: {} → {}", area, game.owners().area(n));
        let built = (1..=game.map().provinces.len() as u16)
            .filter(|&p| game.controller(p, Building::Extraction) == Some(n))
            .any(|p| {
                game.buildings(p).level(Building::Extraction) > 0 || game.buildings(p).level(Building::Factory) > 0
            });
        assert!(built, "bot buduje");
        // Człowiek nic nie robi – jego państwo nie rośnie.
        let human = game.owners().nation_of(0).unwrap();
        assert_eq!(game.economy(human).target_share, crate::economy::DEFAULT_TROOP_SHARE);
    }

    #[test]
    fn bots_pick_a_start_during_selection_and_move_between_neighbours() {
        let mut game = Game::new(config());
        let begin = Intent::BeginSelection { players: vec![0], random: false, bots: vec![10, 11] };
        game.apply_turn(&Turn { tick: 0, intents: vec![StampedIntent { player: SERVER, intent: begin }] });
        run(&mut game, FIRST_PICK_TICKS + HOP_TICKS);
        let first = game.starts()[&10];
        assert!(matches!(game.phase(), Phase::Selection { .. }), "bot widać jeszcze w czasie wyboru");
        let mut seen = vec![first];
        while matches!(game.phase(), Phase::Selection { .. }) {
            let before = game.starts()[&10];
            run(&mut game, 1);
            let now = game.starts()[&10];
            if now != before {
                assert!(game.neighbours(before).iter().any(|l| l.province == now), "przeskok na sąsiednią");
                seen.push(now);
            }
        }
        assert_eq!(game.phase(), Phase::Playing);
        assert!(seen.len() >= 2, "bot idzie do celu przez kilka prowincji: {seen:?}");
        assert_ne!(game.starts()[&10], game.starts()[&11]);
        assert!(game.owners().nation_of(0).is_some(), "człowiek bez wyboru – start losowy");
    }

    #[test]
    fn bots_never_attack_first_and_strike_back() {
        let mut game = with_bots(&[0], &[10]);
        run(&mut game, 4 * crate::economy::TAX_TICKS);
        let n = game.owners().nation_of(10).unwrap();
        assert!(game.hostility.iter().all(|h| h.1 != n), "bot nie wypowiada wojny pierwszy");
        // Gracz wypowiada wojnę w prowincji bota – bot ma uraz i walczy (kontratak, wojsko).
        let human = game.owners().nation_of(0).unwrap();
        let p = game.starts()[&10];
        game.hostility.insert((p, human, n));
        run(&mut game, BOT_PERIOD);
        assert!(game.grudges.contains_key(&(n, human)));
        assert_eq!(game.economy(n).target_share, WAR_SHARE);
        // Napastnik odwołuje wojnę – po `GRUDGE_TICKS` bot zawiera pokój.
        game.hostility.retain(|h| h.1 != human);
        run(&mut game, GRUDGE_TICKS + BOT_PERIOD);
        assert!(!game.grudges.contains_key(&(n, human)));
        assert!(game.hostility.iter().all(|h| !(h.1 == n && h.2 == human)));
    }

    #[test]
    fn player_who_left_is_played_by_a_bot_and_bots_join_late() {
        let mut game = with_bots(&[0, 1], &[]);
        let mut t = Turn { tick: game.tick(), intents: Vec::new() };
        t.intents.push(StampedIntent { player: SERVER, intent: Intent::SetBot { player: 1, bot: true } });
        t.intents.push(StampedIntent { player: SERVER, intent: Intent::SetBot { player: 7, bot: true } });
        // Od gracza – przepada.
        t.intents.push(StampedIntent { player: 0, intent: Intent::SetBot { player: 0, bot: true } });
        game.apply_turn(&t);
        assert!(game.bots().contains(&1) && game.bots().contains(&7) && !game.bots().contains(&0));
        assert!(game.owners().nation_of(7).is_some(), "bot dołączający w grze dostaje start");
        let n = game.owners().nation_of(1).unwrap();
        run(&mut game, BOT_PERIOD * 2);
        assert_ne!(game.economy(n).target_share, crate::economy::DEFAULT_TROOP_SHARE, "bot steruje państwem gracza");
        let mut t = Turn { tick: game.tick(), intents: Vec::new() };
        t.intents.push(StampedIntent { player: SERVER, intent: Intent::SetBot { player: 1, bot: false } });
        game.apply_turn(&t);
        assert!(!game.bots().contains(&1));
    }

    #[test]
    fn games_with_bots_stay_in_sync() {
        let (mut a, mut b) = (with_bots(&[0], &[10, 11, 12]), with_bots(&[0], &[10, 11, 12]));
        for _ in 0..20 {
            run(&mut a, 100);
            run(&mut b, 100);
            assert_eq!(a.state_hash(), b.state_hash());
        }
        let c = with_bots(&[0], &[10, 11]);
        assert_ne!(a.state_hash(), c.state_hash());
    }
}
