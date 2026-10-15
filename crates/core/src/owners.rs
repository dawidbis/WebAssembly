//! Właściciele terenu. GDD: kafel ma jednego właściciela, prowincja może być współdzielona
//! (granica państw może biec przez środek prowincji).
//!
//! Właścicielem kafla jest **państwo** – numer od 1 (0 = kafel niczyj), nadawany graczom w kolejności
//! zdobycia pierwszego kafla. Państw jest najwyżej 255, więc stan to bajt na kafel (tekstura dla
//! renderera bez przeliczania). Państwo pamięta gracza, który nim kieruje (`nations`) – dzięki temu
//! kiedyś da się je przekazać (np. graczowi po ponownym połączeniu) bez zmiany kafli.

use crate::{mapgen::MapData, protocol::PlayerId};

/// Numer państwa (od 1); 0 = niczyje.
pub type Nation = u8;

/// Najwięcej państw na mapie (numer mieści się w bajcie kafla).
pub const MAX_NATIONS: usize = u8::MAX as usize;

#[derive(Clone, Debug, Default)]
pub struct Owners {
    /// Państwo na kafel (0 = niczyje).
    tile: Vec<Nation>,
    /// Liczba kafli państwa `n`: `area[n - 1]` (limit populacji).
    area: Vec<u32>,
    /// Gracz kierujący państwem `n` to `nations[n - 1]`.
    nations: Vec<PlayerId>,
    /// Udziały w prowincji `id`: `shares[id - 1]` = (państwo, liczba kafli), rosnąco po państwie,
    /// tylko niezerowe – pusta lista = prowincja niczyja.
    shares: Vec<Vec<(Nation, u32)>>,
    /// Suma skrótów `(kafel, państwo)` po kaflach z właścicielem – hash stanu bez przechodzenia przez
    /// całą mapę: zmiana kafla odejmuje stary skrót i dodaje nowy (kolejność zmian nie ma znaczenia).
    digest: u64,
    /// Rośnie przy każdej zmianie – UI pobiera kafle tylko po zmianie.
    revision: u32,
}

/// SplitMix64 – skrót kafla z właścicielem do `digest`.
fn mix(tile: usize, nation: Nation) -> u64 {
    let mut z = ((tile as u64) << 8 | nation as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Owners {
    /// Wszystko niczyje.
    pub fn new(map: &MapData) -> Self {
        Self {
            tile: vec![0; map.province.len()],
            nations: Vec::new(),
            area: Vec::new(),
            shares: vec![Vec::new(); map.provinces.len()],
            digest: 0,
            revision: 0,
        }
    }

    /// Państwo na każdym kaflu (0 = niczyje) – w kolejności kafli mapy.
    pub fn tiles(&self) -> &[Nation] {
        &self.tile
    }

    /// Liczba kafli państwa `n` (od 1).
    pub fn area(&self, nation: Nation) -> u32 {
        self.area[nation as usize - 1]
    }

    /// Gracze kierujący państwami: `nations()[n - 1]` dla państwa `n`.
    pub fn nations(&self) -> &[PlayerId] {
        &self.nations
    }

    /// Państwo gracza (`None` – gracz nie ma jeszcze żadnego kafla).
    pub fn nation_of(&self, player: PlayerId) -> Option<Nation> {
        self.nations.iter().position(|&p| p == player).map(|k| k as Nation + 1)
    }

    /// Gracz będący właścicielem kafla.
    pub fn owner(&self, tile: usize) -> Option<PlayerId> {
        match self.tile[tile] {
            0 => None,
            n => Some(self.nations[n as usize - 1]),
        }
    }

    /// Udziały w prowincji (numer od 1): (państwo, liczba kafli), rosnąco po państwie.
    pub fn shares(&self, province: u16) -> &[(Nation, u32)] {
        &self.shares[province as usize - 1]
    }

    /// Udziały wszystkich prowincji (`[id - 1]`).
    pub fn all_shares(&self) -> &[Vec<(Nation, u32)>] {
        &self.shares
    }

    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// Hash stanu właścicieli: skrót kafli i gracze kierujący państwami.
    pub fn hash_bytes(&self) -> impl Iterator<Item = u8> + '_ {
        self.digest.to_le_bytes().into_iter().chain(self.nations.iter().flat_map(|p| p.to_le_bytes()))
    }

    /// Państwo gracza; nowe, jeśli gracz go jeszcze nie ma (`None`, gdy wyczerpano `MAX_NATIONS`).
    pub(crate) fn nation_for(&mut self, player: PlayerId) -> Option<Nation> {
        if let Some(n) = self.nation_of(player) {
            return Some(n);
        }
        if self.nations.len() >= MAX_NATIONS {
            return None;
        }
        self.nations.push(player);
        self.area.push(0);
        Some(self.nations.len() as Nation)
    }

    /// Nowy właściciel kafla `tile` (należącego do prowincji `province`, 0 = poza prowincjami).
    pub(crate) fn set(&mut self, tile: usize, province: u16, nation: Nation) {
        let old = self.tile[tile];
        if old == nation {
            return;
        }
        if old != 0 {
            self.digest = self.digest.wrapping_sub(mix(tile, old));
        }
        if nation != 0 {
            self.digest = self.digest.wrapping_add(mix(tile, nation));
        }
        self.tile[tile] = nation;
        if old != 0 {
            self.area[old as usize - 1] -= 1;
        }
        if nation != 0 {
            self.area[nation as usize - 1] += 1;
        }
        self.revision = self.revision.wrapping_add(1);
        if province == 0 {
            return;
        }
        let shares = &mut self.shares[province as usize - 1];
        if old != 0 {
            let k = shares.iter().position(|s| s.0 == old).expect("udział starego właściciela");
            shares[k].1 -= 1;
            if shares[k].1 == 0 {
                shares.remove(k);
            }
        }
        if nation != 0 {
            match shares.binary_search_by_key(&nation, |s| s.0) {
                Ok(k) => shares[k].1 += 1,
                Err(k) => shares.insert(k, (nation, 1)),
            }
        }
    }
}
