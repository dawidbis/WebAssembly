//! Ataki, desanty morskie i przyczółki.
//!
//! - **Desant** wypływa z naszego portu nad tym samym morzem, z którego droga po kaflach morza do brzegu
//!   celu jest najkrótsza (`sea.rs`), bez limitu odległości; wojska najwyżej `war::LANDING_TROOPS` według
//!   poziomu tego portu.
//! - **Przyczółek**: desant zawsze (a potem ataki z jego terytorium) jest odcięty od kraju (`Attack::expedition`).
//!   Pierwszy zajęty kafel to kafel przyczółka (`Garrison::anchor`); posiłki do odciętego frontu idą tylko
//!   kolejnym desantem (`Attack::reinforce` – po rejsie dołączają do frontu), a po końcu frontu wojsko
//!   zostaje w przyczółku (`Garrison::troops`) – kolejne ataki z przyczółka biorą wojsko stamtąd.
//! - Co `war::EXPEDITION_TICKS` tur przyczółek sprawdza połączenie z krajem: lądem do stolicy albo przez
//!   nasz port w przyczółku. Połączony – wojsko wraca do puli, fronty
//!   przestają być odcięte.

use super::{Game, Phase};
use crate::{
    buildings::Building,
    economy::MILLI,
    goods::{Supply, TICKS_PER_DAY},
    mapgen::{self, Terrain},
    protocol::PlayerId,
    war::{
        self, Attack, AttackBlock, Garrison, GarrisonReport, Landing, LandingReport, MAX_FRONT_PROVINCES, MAX_FRONTS,
        MAX_SEA_LANDINGS, NO_ANCHOR,
    },
};

/// Skąd idzie wojsko na front.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Source {
    /// Z kraju (globalna pula).
    Pool,
    /// Z garnizonu przyczółka (indeks w `Game::garrisons`).
    Garrison(usize),
}

/// Do czego należy nasz kafel styku (`Game::contact_kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Contact {
    Home,
    Garrison(usize),
    Isolated,
}

/// Plan ataku: desant (nowy albo posiłki do odciętego frontu), źródło wojska, posiłki desantem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Plan {
    pub landing: Option<Landing>,
    pub source: Source,
    pub reinforce: bool,
}

/// Nasz port do desantów: prowincja, kafel miejsca portu i morze.
struct OurPort {
    province: u16,
    tile: usize,
    body: u32,
}

impl Game {
    /// Czy gracz może zaatakować prowincje `provinces` jednym frontem (albo dosłać wojsko do trwającego
    /// frontu z którąś z nich – reszta dołącza do niego). Limity według centrum dowodzenia: frontów naraz,
    /// prowincji na froncie, desantów morskich (z portu, w zasięgu, paliwo na rejs). `Ok` – desant albo `None`.
    pub fn check_attack(&self, player: PlayerId, provinces: &[u16]) -> Result<Option<Landing>, AttackBlock> {
        self.attack_plan(player, provinces).map(|p| p.landing)
    }

    pub(super) fn attack_plan(&self, player: PlayerId, provinces: &[u16]) -> Result<Plan, AttackBlock> {
        let Some(n) = self.owners.nation_of(player).filter(|_| self.phase == Phase::Playing) else {
            return Err(AttackBlock::NotPlaying);
        };
        let count = self.map.provinces.len();
        if provinces.is_empty() || provinces.iter().any(|&p| p == 0 || p as usize > count) {
            return Err(AttackBlock::NoContact);
        }
        if provinces.iter().any(|&p| self.map.provinces[p as usize - 1].tunnel > 0) {
            return Err(AttackBlock::Tunnel);
        }
        // Nasza prowincja z armią obrony – dosłanie do niej (nie atak).
        if provinces.iter().any(|&p| self.all_ours(n, p) && self.defense_of(n, p).is_none()) {
            return Err(AttackBlock::Owned);
        }
        let command = self.command_level(n).min(6) as usize;
        // Posiłki płynące desantem nie są osobnym frontem.
        let ours: Vec<&Attack> = self.attacks.iter().filter(|a| a.nation == n && !a.reinforce).collect();
        let with: Vec<&&Attack> = ours.iter().filter(|a| a.provinces.iter().any(|p| provinces.contains(p))).collect();
        if with.len() > 1 {
            return Err(AttackBlock::OtherFront);
        }
        let mut all: Vec<u16> = with.first().map_or(Vec::new(), |a| a.provinces.clone());
        for &p in provinces {
            if !all.contains(&p) {
                all.push(p);
            }
        }
        if all.len() > MAX_FRONT_PROVINCES[command] {
            return Err(AttackBlock::Provinces);
        }
        // Desanty na morzu (wylądowany liczy się już jak zwykły front).
        let landings = self.attacks.iter().filter(|a| a.nation == n && a.landing.is_some() && a.sailing > 0).count();
        let plan = match with.first() {
            // Odcięty przyczółek – posiłki tylko kolejnym desantem.
            Some(a) if a.expedition => {
                if landings >= MAX_SEA_LANDINGS[command] {
                    return Err(AttackBlock::Landing);
                }
                Plan { landing: Some(self.sea_landing(n, &a.provinces)?), source: Source::Pool, reinforce: true }
            }
            Some(_) => Plan { landing: None, source: Source::Pool, reinforce: false },
            None => {
                if ours.iter().filter(|a| !a.defense).count() >= MAX_FRONTS[command] {
                    return Err(AttackBlock::Limit);
                }
                match self.land_source(n, provinces) {
                    Some(source) => Plan { landing: None, source, reinforce: false },
                    None if provinces.iter().any(|&p| self.touches_sea(p)) => {
                        if landings >= MAX_SEA_LANDINGS[command] {
                            return Err(AttackBlock::Landing);
                        }
                        Plan { landing: Some(self.sea_landing(n, provinces)?), source: Source::Pool, reinforce: false }
                    }
                    None => return Err(AttackBlock::NoContact),
                }
            }
        };
        let available = match plan.source {
            Source::Pool => self.economy[n as usize - 1].troops,
            Source::Garrison(g) => self.garrisons[g].troops,
        };
        if available < war::MIN_ATTACK_TROOPS * MILLI {
            return Err(if plan.source == Source::Pool { AttackBlock::NoTroops } else { AttackBlock::Cutoff });
        }
        Ok(plan)
    }

    /// Wojsko, które może pójść na front z planem `plan` (tysięczne osoby).
    pub(super) fn plan_troops(&self, n: u8, plan: &Plan) -> u64 {
        match plan.source {
            Source::Pool => self.economy[n as usize - 1].troops,
            Source::Garrison(g) => self.garrisons[g].troops,
        }
    }

    /// Desant na prowincje z najbliższego portu w zasięgu, z paliwem na rejs.
    fn sea_landing(&self, n: u8, provinces: &[u16]) -> Result<Landing, AttackBlock> {
        let landing = self.landing_route(n, provinces)?;
        let fuel = self.economy[n as usize - 1].supplies[Supply::Fuel as usize];
        if landing.troops_for(fuel) < war::MIN_ATTACK_TROOPS * MILLI {
            return Err(AttackBlock::NoFuel);
        }
        Ok(landing)
    }

    /// Styk lądem (bokiem, przez wąską rzekę albo z naszego brzegu przez wodę) z którąś z prowincji i skąd
    /// idzie wojsko: z kraju, gdy choć jeden nasz kafel styku jest poza odciętymi przyczółkami, inaczej z
    /// garnizonu przyczółka. Przez wodę stykają się nasze kafle nad tym samym zbiornikiem (nad morzem – tylko
    /// w tej prowincji, jak w `frontier`). `None` – brak styku.
    fn land_source(&self, n: u8, provinces: &[u16]) -> Option<Source> {
        let cut = self.garrisons.iter().any(|g| g.nation == n);
        let (w, h) = (self.map.width as usize, self.map.height as usize);
        let mut garrison = None;
        let mut any = false;
        let mut memo = Vec::new();
        // Zbiorniki styku przez wodę: (zbiornik, prowincja celu).
        let mut shores: Vec<(u32, u16)> = Vec::new();
        for &p in provinces {
            for (t, _, water) in self.frontier(n, p, None) {
                any = true;
                // Bez przyczółków każdy nasz kafel to kraj.
                if !cut {
                    return Some(Source::Pool);
                }
                if !water {
                    for s in self.contact_tiles(n, t) {
                        match self.contact_kind(n, s, &mut memo) {
                            Contact::Home => return Some(Source::Pool),
                            Contact::Garrison(g) => garrison = garrison.or(Some(g)),
                            Contact::Isolated => {}
                        }
                    }
                    continue;
                }
                let (x, y) = (t % w, t / w);
                for (ok, s) in
                    [(x > 0, t.wrapping_sub(1)), (x + 1 < w, t + 1), (y > 0, t.wrapping_sub(w)), (y + 1 < h, t + w)]
                {
                    let b = if ok { self.water.body[s] } else { 0 };
                    if b != 0 && !shores.contains(&(b, p)) {
                        shores.push((b, p));
                    }
                }
            }
        }
        let tiles = self.owners.tiles();
        for (b, p) in shores {
            let sea = self.water.sea[b as usize - 1];
            for &c in &self.water.coast[b as usize - 1] {
                let c = c as usize;
                if tiles[c] != n || (sea && self.map.province[c] != p) {
                    continue;
                }
                match self.contact_kind(n, c, &mut memo) {
                    Contact::Home => return Some(Source::Pool),
                    Contact::Garrison(g) => garrison = garrison.or(Some(g)),
                    Contact::Isolated => {}
                }
            }
        }
        // Styk tylko z odciętymi kawałkami bez garnizonu – jakby go nie było.
        garrison.map(Source::Garrison).or((any && !cut).then_some(Source::Pool))
    }

    /// Nasze kafle stykające się z kaflem `t` (bokiem albo w linii prostej przez wąską rzekę).
    pub(super) fn contact_tiles(&self, n: u8, t: usize) -> Vec<usize> {
        let (w, h) = (self.map.width as i64, self.map.height as i64);
        let tiles = self.owners.tiles();
        let (x, y) = (t as i64 % w, t as i64 / w);
        let mut out = Vec::new();
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (mut cx, mut cy) = (x + dx, y + dy);
            for _ in 0..=war::RIVER_CROSSING_TILES {
                if cx < 0 || cy < 0 || cx >= w || cy >= h {
                    break;
                }
                let k = (cy * w + cx) as usize;
                if tiles[k] == n {
                    out.push(k);
                    break;
                }
                if self.map.terrain[k] != Terrain::River as u8 {
                    break;
                }
                (cx, cy) = (cx + dx, cy + dy);
            }
        }
        out
    }

    /// Do czego należy nasz kafel `t`: do kraju, do przyczółka z garnizonem albo do odciętego kawałka bez
    /// garnizonu. Bez przyczółków – zawsze kraj. Z kaflami zajętymi od ostatniego sprawdzenia: przejście po
    /// naszych kaflach od `t` do znanego kafla garnizonu albo do stolicy (albo portu połączonego z krajem).
    /// `memo` (na początku puste) pamięta wynik dla kafli przejścia – wszystkie należą do tego samego.
    pub(super) fn contact_kind(&self, n: u8, t: usize, memo: &mut Vec<Option<Contact>>) -> Contact {
        if !self.garrisons.iter().any(|g| g.nation == n) {
            return Contact::Home;
        }
        if let Some(g) = self.garrison_of(n, t) {
            return Contact::Garrison(g);
        }
        if let Some(Some(k)) = memo.get(t) {
            return *k;
        }
        let capital = self.capital_tile(n);
        let (seen, tiles, at) = self.walk(n, t, |i| Some(i) == capital || self.garrison_of(n, i).is_some());
        let kind = match at {
            Some(i) if Some(i) == capital => Contact::Home,
            Some(i) => self.garrison_of(n, i).map_or(Contact::Isolated, Contact::Garrison),
            None if self.linked(n, &seen) => Contact::Home,
            None => Contact::Isolated,
        };
        if memo.is_empty() {
            *memo = vec![None; seen.len()];
        }
        for i in tiles {
            memo[i as usize] = Some(kind);
        }
        kind
    }

    pub(super) fn garrison_of(&self, n: u8, t: usize) -> Option<usize> {
        self.garrisons.iter().position(|g| g.nation == n && self.garrison_tiles(g).binary_search(&(t as u32)).is_ok())
    }

    /// Kafle odciętego kawałka terytorium garnizonu (z ostatniego sprawdzenia; sam kafel przyczółka, zanim
    /// pierwsze sprawdzenie się odbyło).
    fn garrison_tiles<'a>(&'a self, g: &'a Garrison) -> &'a [u32] {
        if g.tiles.is_empty() { std::slice::from_ref(&g.anchor) } else { &g.tiles }
    }

    /// Desant dla interfejsu: port, czas rejsu, paliwo i limity wojska.
    pub fn landing_report(&self, nation: u8, l: &Landing) -> LandingReport {
        let fuel = self.economy[nation as usize - 1].supplies[Supply::Fuel as usize];
        LandingReport {
            port: l.port,
            days: l.sailing() as f64 / TICKS_PER_DAY as f64,
            fuel_per_1000: l.fuel(1000 * MILLI) as f64 / MILLI as f64,
            max_troops: (self.landing_troops(l) / MILLI) as f64,
            fuel_troops: (l.troops_for(fuel) / MILLI) as f64,
        }
    }

    /// Najwięcej wojska na desancie (tysięczne osoby) – według poziomu portu wypłynięcia.
    pub(super) fn landing_troops(&self, l: &Landing) -> u64 {
        let level = self.buildings(l.port).level(Building::Port).min(3) as usize;
        war::LANDING_TROOPS[level] * MILLI
    }

    /// Czy prowincja styka się z morzem.
    pub(super) fn touches_sea(&self, province: u16) -> bool {
        self.water.province_bodies[province as usize - 1].iter().any(|&b| self.water.sea[b as usize - 1])
    }

    /// Nasze porty (kontrolowane, z miejscem nad morzem).
    fn our_ports(&self, n: u8) -> Vec<OurPort> {
        let (w, h) = (self.map.width as usize, self.map.height as usize);
        (1..=self.map.provinces.len() as u16)
            .filter_map(|q| {
                let level = self.buildings(q).level(Building::Port);
                if level == 0 || self.controller(q, Building::Port) != Some(n) {
                    return None;
                }
                let [x, y] = self.map.provinces[q as usize - 1].slots[mapgen::slots::SLOT_PORT];
                if [x, y] == mapgen::slots::NO_SLOT {
                    return None;
                }
                let tile = y as usize * w + x as usize;
                let body = self.water.sea_next_to(tile, w, h)?;
                Some(OurPort { province: q, tile, body })
            })
            .collect()
    }

    /// Rejs desantu na którąś z prowincji: droga po kaflach morza z naszego portu do kafla morza przy brzegu
    /// celu – najkrótsza. `NoPort` – żadnego portu nad tym morzem albo brak drogi.
    pub(super) fn landing_route(&self, n: u8, provinces: &[u16]) -> Result<Landing, AttackBlock> {
        let (w, h) = (self.map.width as usize, self.map.height as usize);
        // Brzeg celu nad morzami: (morze, kafel lądu, kafel morza przy nim).
        let mut shore: Vec<(u32, usize, usize)> = Vec::new();
        for &p in provinces {
            for &b in &self.water.province_bodies[p as usize - 1] {
                if !self.water.sea[b as usize - 1] {
                    continue;
                }
                for &t in self.water.coast[b as usize - 1].iter().filter(|&&t| self.map.province[t as usize] == p) {
                    let t = t as usize;
                    let (x, y) = (t % w, t / w);
                    // Lądowanie tylko na kaflu, który styka się bokiem z resztą prowincji (front pójdzie dalej lądem).
                    let inland = [
                        (x > 0, t.wrapping_sub(1)),
                        (x + 1 < w, t + 1),
                        (y > 0, t.wrapping_sub(w)),
                        (y + 1 < h, t + w),
                    ]
                    .into_iter()
                    .any(|(ok, j)| ok && self.map.province[j] == p);
                    if !inland {
                        continue;
                    }
                    for (ok, s) in
                        [(x > 0, t.wrapping_sub(1)), (x + 1 < w, t + 1), (y > 0, t.wrapping_sub(w)), (y + 1 < h, t + w)]
                    {
                        if ok && self.water.body[s] == b {
                            shore.push((b, t, s));
                        }
                    }
                }
            }
        }
        let cheb = |a: usize, b: usize| ((a % w).abs_diff(b % w)).max((a / w).abs_diff(b / w)) as u32;
        // Porty od najbliższego w linii prostej (dolne ograniczenie drogi) – dalsze odpadają bez liczenia.
        let mut ports: Vec<(u32, OurPort)> = self
            .our_ports(n)
            .into_iter()
            .filter_map(|p| {
                let bound = shore.iter().filter(|s| s.0 == p.body).map(|s| cheb(p.tile, s.2)).min()?;
                Some((bound, p))
            })
            .collect();
        ports.sort_by_key(|(bound, p)| (*bound, p.province));
        let rank = |l: &Landing| (l.distance, l.port, l.to);
        let mut best: Option<Landing> = None;
        for (bound, port) in &ports {
            if best.as_ref().is_some_and(|b| *bound >= b.distance) {
                break;
            }
            let Some(field) = self.sea_field(port.tile) else { continue };
            for &(b, t, s) in shore.iter().filter(|s| s.0 == port.body) {
                let Some(distance) = self.sea_distance(&field, s) else { continue };
                let l = Landing {
                    body: b,
                    port: port.province,
                    distance,
                    from: port.tile as u32,
                    to: t as u32,
                    sea: s as u32,
                };
                if best.as_ref().is_none_or(|x| rank(&l) < rank(x)) {
                    best = Some(l);
                }
            }
        }
        best.ok_or(AttackBlock::NoPort)
    }

    /// Atak: `percent` wojska ze źródła planu (kraj albo garnizon przyczółka) idzie na front prowincji
    /// `provinces` (do trwającego frontu z którąś z nich – dosłane, a nowe prowincje dołączają do niego;
    /// do odciętego – desantem).
    pub(super) fn attack(&mut self, player: PlayerId, provinces: &[u16], percent: u8) {
        let Ok(plan) = self.attack_plan(player, provinces) else { return };
        let n = self.owners.nation_of(player).unwrap();
        let mut troops = self.plan_troops(n, &plan) * percent.min(100) as u64 / 100;
        let fuel_stock = self.economy[n as usize - 1].supplies[Supply::Fuel as usize];
        if let Some(l) = plan.landing {
            troops = troops.min(self.landing_troops(&l)).min(l.troops_for(fuel_stock));
        }
        if troops < war::MIN_ATTACK_TROOPS * MILLI {
            return;
        }
        let e = &mut self.economy[n as usize - 1];
        if let Some(l) = plan.landing {
            let fuel = l.fuel(troops).min(e.supplies[Supply::Fuel as usize]);
            e.supplies[Supply::Fuel as usize] -= fuel;
            e.supply_use[Supply::Fuel as usize] += fuel;
        }
        match plan.source {
            Source::Pool => {
                e.troops -= troops;
                e.deployed += troops;
            }
            // Garnizon jest już wojskiem poza krajem (`deployed`).
            Source::Garrison(g) => self.garrisons[g].troops -= troops,
        }
        let mut targets: Vec<u16> = Vec::new();
        for &p in provinces {
            if !targets.contains(&p) {
                targets.push(p);
            }
        }
        self.declare_on_attack(n, &targets);
        let running = self
            .attacks
            .iter()
            .position(|a| a.nation == n && !a.reinforce && a.provinces.iter().any(|p| targets.contains(p)));
        let path = plan.landing.map_or(Vec::new(), |l| self.sea_route(l.from as usize, l.sea as usize));
        match (running, plan.reinforce) {
            (Some(k), false) => {
                let a = &mut self.attacks[k];
                a.troops += troops;
                a.sent += troops;
                a.idle = 0;
                // Armia obrony zostaje przy swojej prowincji.
                for p in targets.into_iter().filter(|_| !a.defense) {
                    if !a.provinces.contains(&p) {
                        a.provinces.push(p);
                    }
                }
            }
            _ => {
                let (anchor, provinces) = match running {
                    Some(k) => (self.attacks[k].anchor, self.attacks[k].provinces.clone()),
                    None => (
                        match plan.source {
                            Source::Garrison(g) => self.garrisons[g].anchor,
                            Source::Pool => NO_ANCHOR,
                        },
                        targets,
                    ),
                };
                self.attacks.push(Attack {
                    nation: n,
                    provinces,
                    troops,
                    taken: 0,
                    supply: war::NOT_FIGHTING,
                    landing: plan.landing,
                    sailing: plan.landing.map_or(0, |l| l.sailing()),
                    lost: 0,
                    thrusts: Vec::new(),
                    pace: 100,
                    front: 0,
                    strength: 100,
                    fuel_pct: 0,
                    horses: false,
                    sent: troops,
                    idle: 0,
                    // Desant i atak z przyczółka – odcięte od kraju.
                    expedition: plan.landing.is_some() || matches!(plan.source, Source::Garrison(_)),
                    anchor,
                    reinforce: plan.reinforce,
                    defense: false,
                    path,
                });
            }
        }
    }

    /// Odwołanie frontu z prowincją `province`: wojsko wraca do kraju, z odciętego przyczółka – do jego
    /// garnizonu (desant jeszcze na morzu zawraca do kraju).
    pub(super) fn cancel_attack(&mut self, player: PlayerId, province: u16) {
        let Some(n) = self.owners.nation_of(player) else { return };
        let Some(k) = self
            .attacks
            .iter()
            .position(|a| a.nation == n && !a.reinforce && !a.defense && a.provinces.contains(&province))
        else {
            return;
        };
        let a = self.attacks.remove(k);
        self.settle(a);
    }

    /// Wojsko zakończonego (albo odwołanego) frontu: z odciętego przyczółka (po rejsie) zostaje w jego
    /// garnizonie, inaczej wraca do kraju. Przyczółek stracony – wojsko przepada.
    pub(super) fn settle(&mut self, a: Attack) {
        let n = a.nation;
        if a.expedition && a.sailing == 0 && a.troops > 0 {
            let tiles = self.owners.tiles();
            let anchor = if a.anchor != NO_ANCHOR && tiles[a.anchor as usize] == n {
                Some(a.anchor as usize)
            } else {
                // Inny nasz kafel w prowincjach frontu.
                a.provinces.iter().find_map(|&p| self.tile_of(n, p))
            };
            match anchor {
                Some(t) => self.garrison_at(n, t).troops += a.troops,
                None => self.economy[n as usize - 1].deployed -= a.troops,
            }
            return;
        }
        let e = &mut self.economy[n as usize - 1];
        e.deployed -= a.troops;
        e.troops += a.troops;
    }

    /// Nasz kafel w prowincji (pierwszy).
    fn tile_of(&self, n: u8, province: u16) -> Option<usize> {
        let [x0, y0, x1, y1] = self.bounds[province as usize - 1];
        let w = self.map.width as usize;
        let tiles = self.owners.tiles();
        (y0 as usize..=y1 as usize)
            .flat_map(|y| (x0 as usize..=x1 as usize).map(move |x| y * w + x))
            .find(|&i| self.map.province[i] == province && tiles[i] == n)
    }

    /// Garnizon przyczółka z kaflem `t` (istniejący albo nowy, z `t` jako kaflem przyczółka).
    pub(super) fn garrison_at(&mut self, n: u8, t: usize) -> &mut Garrison {
        let k = match self.garrison_of(n, t) {
            Some(k) => k,
            None => {
                self.garrisons.push(Garrison { nation: n, anchor: t as u32, troops: 0, tiles: Vec::new() });
                self.garrisons.len() - 1
            }
        };
        &mut self.garrisons[k]
    }

    /// Posiłki desantem po rejsie: dołączają do odciętego frontu tych prowincji (`true` – dołączyły i znikają).
    /// Gdy frontu już nie ma, płyną dalej jako zwykły desant.
    pub(super) fn reinforce_arrived(&mut self, i: usize) -> bool {
        let a = &self.attacks[i];
        let target = self
            .attacks
            .iter()
            .position(|b| b.nation == a.nation && !b.reinforce && b.provinces.iter().any(|p| a.provinces.contains(p)));
        match target {
            Some(j) => {
                let troops = self.attacks[i].troops;
                let b = &mut self.attacks[j];
                b.troops += troops;
                b.sent += troops;
                b.idle = 0;
                self.attacks[i].troops = 0;
                true
            }
            None => {
                self.attacks[i].reinforce = false;
                false
            }
        }
    }

    /// Co `war::EXPEDITION_TICKS` tur: przyczółki sprawdzają połączenie z krajem. Połączony – wojsko garnizonu
    /// wraca do puli, a fronty z tego terytorium nie są już odcięte; odcięty – zapamiętuje swoje kafle;
    /// stracony (żaden kafel nie nasz) – garnizon przepada. Dwa garnizony w jednym kawałku łączą się.
    pub(super) fn update_garrisons(&mut self) {
        let mut k = 0;
        while k < self.garrisons.len() {
            let n = self.garrisons[k].nation;
            let tiles = self.owners.tiles();
            let anchor = if tiles[self.garrisons[k].anchor as usize] == n {
                Some(self.garrisons[k].anchor as usize)
            } else {
                self.garrisons[k].tiles.iter().map(|&t| t as usize).find(|&t| tiles[t] == n)
            };
            let Some(anchor) = anchor else {
                let g = self.garrisons.remove(k);
                self.economy[n as usize - 1].deployed -= g.troops;
                continue;
            };
            let (component, connected) = self.territory_from(n, anchor);
            if connected {
                let g = self.garrisons.remove(k);
                let e = &mut self.economy[n as usize - 1];
                e.deployed -= g.troops;
                e.troops += g.troops;
                for a in self.attacks.iter_mut().filter(|a| a.nation == n && a.sailing == 0) {
                    if a.anchor != NO_ANCHOR && component.binary_search(&a.anchor).is_ok() {
                        a.expedition = false;
                    }
                }
                continue;
            }
            // Wcześniejszy garnizon w tym samym kawałku – połączenie.
            if let Some(j) = (0..k)
                .find(|&j| self.garrisons[j].nation == n && component.binary_search(&self.garrisons[j].anchor).is_ok())
            {
                let g = self.garrisons.remove(k);
                self.garrisons[j].troops += g.troops;
                for a in self.attacks.iter_mut().filter(|a| a.nation == n && a.anchor == g.anchor) {
                    a.anchor = self.garrisons[j].anchor;
                }
                continue;
            }
            let g = &mut self.garrisons[k];
            g.anchor = anchor as u32;
            g.tiles = component;
            k += 1;
        }
    }

    /// Kawałek terytorium państwa `n` z kafla `start` (po własnych kaflach bokiem i przez wąską rzekę,
    /// rosnąco) i czy jest połączony z krajem: ma kafel stolicy albo nasz port.
    fn territory_from(&self, n: u8, start: usize) -> (Vec<u32>, bool) {
        let capital = self.capital_tile(n);
        let (seen, mut out, _) = self.walk(n, start, |_| false);
        out.sort_unstable();
        if capital.is_some_and(|c| seen[c]) {
            return (out, true);
        }
        let linked = self.linked(n, &seen);
        (out, linked)
    }

    /// Czy odcięty lądem kawałek terytorium (`seen` – maska) jest połączony z krajem morzem: stoi w nim zdobyty
    /// nasz port (budynek).
    fn linked(&self, n: u8, seen: &[bool]) -> bool {
        self.our_ports(n).iter().any(|p| seen[p.tile])
    }

    /// Przejście po kaflach państwa `n` od `start` (bokiem i przez wąską rzekę): odwiedzone (maska i lista)
    /// i kafel, na którym `stop` przerwało przejście.
    pub(super) fn walk(
        &self,
        n: u8,
        start: usize,
        stop: impl Fn(usize) -> bool,
    ) -> (Vec<bool>, Vec<u32>, Option<usize>) {
        let (w, h) = (self.map.width as i64, self.map.height as i64);
        let tiles = self.owners.tiles();
        let mut seen = vec![false; tiles.len()];
        let mut stack = vec![start];
        let mut out = Vec::new();
        seen[start] = true;
        while let Some(i) = stack.pop() {
            out.push(i as u32);
            if stop(i) {
                return (seen, out, Some(i));
            }
            let (x, y) = (i as i64 % w, i as i64 / w);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (mut cx, mut cy) = (x + dx, y + dy);
                for _ in 0..=war::RIVER_CROSSING_TILES {
                    if cx < 0 || cy < 0 || cx >= w || cy >= h {
                        break;
                    }
                    let k = (cy * w + cx) as usize;
                    if tiles[k] == n {
                        if !seen[k] {
                            seen[k] = true;
                            stack.push(k);
                        }
                        break;
                    }
                    if self.map.terrain[k] != Terrain::River as u8 {
                        break;
                    }
                    (cx, cy) = (cx + dx, cy + dy);
                }
            }
        }
        (seen, out, None)
    }

    /// Czy nasze kafle prowincji należą do odciętego przyczółka (z ostatniego sprawdzenia).
    pub fn cut_off(&self, n: u8, province: u16) -> bool {
        self.garrisons
            .iter()
            .filter(|g| g.nation == n)
            .any(|g| self.garrison_tiles(g).iter().any(|&t| self.map.province[t as usize] == province))
    }

    /// Obrona kafla `t` państwa `owner`, gdy leży w odciętym przyczółku: garnizon i wojsko frontów z tego
    /// przyczółka (na kafel przyczółka) – nie cała pula kraju. `None` – kafel poza przyczółkiem.
    pub(super) fn bridgehead_defense(&self, owner: u8, t: usize) -> Option<(usize, u64)> {
        let g = self.garrison_of(owner, t)?;
        let tiles = self.garrison_tiles(&self.garrisons[g]).len().max(1) as u64;
        Some((g, self.bridgehead_troops(owner, g) / tiles))
    }

    /// Wojsko broniące przyczółka `g`: garnizon i fronty z tego przyczółka (tysięczne osoby).
    fn bridgehead_troops(&self, owner: u8, g: usize) -> u64 {
        let tiles = self.garrison_tiles(&self.garrisons[g]);
        let fronts: u64 = self
            .attacks
            .iter()
            .filter(|a| a.nation == owner && a.sailing == 0 && tiles.binary_search(&a.anchor).is_ok())
            .map(|a| a.troops)
            .sum();
        self.garrisons[g].troops + fronts
    }

    /// Wojsko obrońcy `owner` na froncie z kaflami `front`: w przyczółku – jego obrońcy, inaczej armie obrony
    /// prowincji frontu (`Attack::defense`).
    pub(super) fn defender_troops(&self, owner: u8, front: impl Iterator<Item = usize>) -> u64 {
        let tiles = self.owners.tiles();
        let mut provinces: Vec<u16> = Vec::new();
        for t in front.filter(|&t| tiles[t] == owner) {
            if let Some(g) = self.garrison_of(owner, t) {
                return self.bridgehead_troops(owner, g);
            }
            let p = self.map.province[t];
            if !provinces.contains(&p) {
                provinces.push(p);
            }
        }
        provinces.iter().filter_map(|&p| self.defense_of(owner, p)).map(|k| self.attacks[k].troops).sum()
    }

    /// Straty obrońcy przyczółka `g`: najpierw garnizon, potem fronty z przyczółka.
    pub(super) fn bridgehead_losses(&mut self, owner: u8, g: usize, dead: u64) {
        let take = dead.min(self.garrisons[g].troops);
        self.garrisons[g].troops -= take;
        let (mut left, mut killed) = (dead - take, take);
        let tiles = self.garrison_tiles(&self.garrisons[g]).to_vec();
        for a in self.attacks.iter_mut() {
            if left == 0 {
                break;
            }
            if a.nation == owner && a.sailing == 0 && tiles.binary_search(&a.anchor).is_ok() {
                let d = left.min(a.troops);
                a.troops -= d;
                a.lost += d;
                left -= d;
                killed += d;
            }
        }
        // Garnizon i fronty to wojsko poza krajem (`deployed`).
        self.economy[owner as usize - 1].deployed -= killed;
    }

    /// Kafle odciętych przyczółków wszystkich państw (rosnąco, z ostatniego sprawdzenia, wciąż ich) –
    /// wykreskowane na mapie politycznej.
    pub fn bridgehead_tiles(&self) -> Vec<u32> {
        let tiles = self.owners.tiles();
        let mut out: Vec<u32> = self
            .garrisons
            .iter()
            .flat_map(|g| self.garrison_tiles(g).iter().copied().filter(move |&t| tiles[t as usize] == g.nation))
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Garnizony przyczółków gracza `viewer` (prowincja kafla przyczółka i wojsko).
    pub fn garrisons(&self, viewer: PlayerId) -> Vec<GarrisonReport> {
        let Some(n) = self.owners.nation_of(viewer) else { return Vec::new() };
        let w = self.map.width;
        self.garrisons
            .iter()
            .filter(|g| g.nation == n)
            .map(|g| GarrisonReport {
                province: self.map.province[g.anchor as usize],
                troops: (g.troops / MILLI) as f64,
                x: g.anchor % w,
                y: g.anchor / w,
            })
            .collect()
    }
}
