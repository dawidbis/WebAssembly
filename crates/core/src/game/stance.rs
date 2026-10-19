//! Pokój i wojna w prowincji między dwoma państwami.
//!
//! - Domyślnie pokój: fronty dwóch państw w tej samej prowincji (np. wyścig o prowincję tubylców) nie
//!   zajmują sobie kafli i nie ścierają się (`Game::opposed`).
//! - Wojnę wypowiada się w prowincji jednemu udziałowcowi (`Intent::SetStance`, w oknie prowincji – miecz).
//!   Działa w obie strony: zaatakowany ma od razu miecz i nie może wrócić do pokoju, dopóki napastnik nie
//!   odwoła wojny.
//! - Atak na prowincję bez kafli niczyich (cudza ziemia) to wojna z jej właścicielami w tej prowincji – bez
//!   osobnego wypowiedzenia.

use super::{Game, Phase};
use crate::{protocol::PlayerId, war::StanceReport};

impl Game {
    /// Czy państwa `a` i `b` walczą w prowincji (któreś wypowiedziało drugiemu wojnę).
    pub(super) fn at_war(&self, a: u8, b: u8, province: u16) -> bool {
        self.hostility.contains(&(province, a, b)) || self.hostility.contains(&(province, b, a))
    }

    /// Wypowiedzenie (`war`) albo odwołanie wojny graczowi `other` w prowincji – tylko w grze właściwej,
    /// gdy `other` ma w niej kafle.
    pub(super) fn set_stance(&mut self, player: PlayerId, province: u16, other: PlayerId, war: bool) {
        let (Some(n), Some(m)) = (self.owners.nation_of(player), self.owners.nation_of(other)) else { return };
        if self.phase != Phase::Playing || n == m || province == 0 || province as usize > self.map.provinces.len() {
            return;
        }
        if war {
            if self.owners.shares(province).iter().any(|s| s.0 == m) {
                self.hostility.insert((province, n, m));
            }
        } else {
            self.hostility.remove(&(province, n, m));
        }
    }

    /// Atak państwa `n` na prowincje: w tych bez kafli niczyich – wojna z ich właścicielami (inwazja).
    pub(super) fn declare_on_attack(&mut self, n: u8, provinces: &[u16]) {
        for &p in provinces {
            let shares = self.owners.shares(p);
            let held: u32 = shares.iter().map(|s| s.1).sum();
            if held < self.map.provinces[p as usize - 1].area {
                continue;
            }
            let others: Vec<u8> = shares.iter().map(|s| s.0).filter(|&m| m != n).collect();
            for m in others {
                self.hostility.insert((p, n, m));
            }
        }
    }

    /// Pokój albo wojna oglądającego z innymi udziałowcami prowincji.
    pub(super) fn stance_report(&self, viewer: PlayerId, province: u16) -> Vec<StanceReport> {
        let Some(n) = self.owners.nation_of(viewer).filter(|_| self.phase == Phase::Playing) else { return Vec::new() };
        self.owners
            .shares(province)
            .iter()
            .filter(|s| s.0 != n)
            .map(|&(m, _)| {
                let declared = self.hostility.contains(&(province, n, m));
                let attacked = self.hostility.contains(&(province, m, n));
                StanceReport {
                    player: self.owners.nations()[m as usize - 1],
                    war: declared || attacked,
                    declared,
                    attacked,
                }
            })
            .collect()
    }
}
