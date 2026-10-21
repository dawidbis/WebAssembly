//! Armie obrony.
//!
//! - Gdy wrogi front (w wojnie; także desant w rejsie i przeprawa w przygotowaniu) idzie na prowincję, każdy
//!   jej udziałowiec w wojnie z napastnikiem wystawia w niej armię obrony (`Attack::defense`, jedna na państwo
//!   i prowincję): `war::DEFENSE_SHARE_PCT`% limitu populacji, który daje mu ta prowincja, z puli kraju (mniej,
//!   gdy pula mniejsza).
//! - Armia obrony to zwykły front tej prowincji: ściera się z napastnikiem (`Game::opposed`), broni kafli (jej
//!   gęstość – `Game::defense`), zużywa zaopatrzenie i paliwo (`war::DEFENSE_SUPPLY_PCT`%) i odbija swoje kafle
//!   w tej prowincji (ziemi niczyjej nie zajmuje).
//! - Bez uzupełniania: wybita – kafle bronią się minimalnie; dosłanie wojska – `Intent::Attack` na prowincję.
//! - Koniec, gdy nikt już o prowincję nie walczy (albo obrońca nie ma w niej kafli) – wojsko wraca do puli.
//! - Kafli odciętego przyczółka broni jego garnizon (`Game::bridgehead_defense`) – tam armii obrony nie ma.

use super::Game;
use crate::{
    buildings::{self, Building, CITY_POPULATION},
    economy::{MILLI, PEOPLE_PER_TILE},
    war::{self, Attack, NO_ANCHOR},
};

impl Game {
    /// Armia obrony państwa `nation` w prowincji (numer w `Game::attacks`).
    pub(super) fn defense_of(&self, nation: u8, province: u16) -> Option<usize> {
        self.attacks.iter().position(|a| a.defense && a.nation == nation && a.provinces.first() == Some(&province))
    }

    /// Limit populacji, który prowincja daje państwu (tysięczne osoby): jego kafle × `PEOPLE_PER_TILE` i miasto,
    /// jeśli je kontroluje.
    pub(super) fn province_cap(&self, nation: u8, province: u16) -> u64 {
        let tiles = self.owners.shares(province).iter().find(|s| s.0 == nation).map_or(0, |s| s.1) as u64;
        let city = if self.controller(province, Building::City) == Some(nation) {
            self.buildings(province).level(Building::City) as u64
        } else {
            0
        };
        (tiles * PEOPLE_PER_TILE + buildings::level_total(city as u8, CITY_POPULATION)) * MILLI
    }

    /// Wojsko, które wystawi armia obrony (tysięczne osoby): `DEFENSE_SHARE_PCT`% limitu prowincji, najwyżej pula.
    pub(super) fn defense_levy(&self, nation: u8, province: u16) -> u64 {
        (self.province_cap(nation, province) * war::DEFENSE_SHARE_PCT / 100)
            .min(self.economy[nation as usize - 1].troops)
    }

    /// Czy ktoś (front w wojnie z `nation` w prowincji – także w rejsie i w przeprawie) idzie na prowincję.
    fn contested(&self, nation: u8, province: u16) -> bool {
        self.attacks.iter().any(|a| {
            !a.defense
                && a.nation != nation
                && a.provinces.contains(&province)
                && self.at_war(a.nation, nation, province)
        })
    }

    /// Co turę przed walką: kończy armie obrony prowincji, o które nikt już nie walczy (wojsko wraca do puli),
    /// i wystawia nowe tam, gdzie zaczęła się walka.
    pub(super) fn update_defenses(&mut self) {
        let mut k = 0;
        while k < self.attacks.len() {
            let a = &self.attacks[k];
            let gone = a.defense && {
                let p = a.provinces[0];
                !self.contested(a.nation, p) || !self.owners.shares(p).iter().any(|s| s.0 == a.nation)
            };
            if gone {
                let a = self.attacks.remove(k);
                self.settle(a);
            } else {
                k += 1;
            }
        }
        let mut wanted: Vec<(u8, u16)> = Vec::new();
        for a in self.attacks.iter().filter(|a| !a.defense) {
            for &p in &a.provinces {
                for &(d, _) in self.owners.shares(p) {
                    let new = d != a.nation
                        && !self.annexed.contains_key(&d)
                        && self.at_war(a.nation, d, p)
                        && !self.cut_off(d, p)
                        && !wanted.contains(&(d, p));
                    if new && self.defense_of(d, p).is_none() {
                        wanted.push((d, p));
                    }
                }
            }
        }
        for (d, p) in wanted {
            let troops = self.defense_levy(d, p);
            let e = &mut self.economy[d as usize - 1];
            e.troops -= troops;
            e.deployed += troops;
            self.attacks.push(Attack {
                nation: d,
                provinces: vec![p],
                troops,
                taken: 0,
                supply: war::NOT_FIGHTING,
                landing: None,
                crossing: None,
                sailing: 0,
                lost: 0,
                thrusts: Vec::new(),
                pace: 100,
                front: 0,
                strength: 100,
                fuel_pct: 0,
                horses: false,
                sent: troops,
                idle: 0,
                expedition: false,
                anchor: NO_ANCHOR,
                reinforce: false,
                defense: true,
                path: Vec::new(),
            });
        }
    }
}
