//! Trasy morskie desantów: droga po kaflach morza od portu. Ruch na 8 kierunków (prosto `STEP`, na skos
//! `DIAG` – ok. √2 razy dłużej), na skos tylko między dwoma kaflami tego samego zbiornika (bez przecinania
//! cypli). Droga trzyma się z dala od lądu, gdy się da: kafel bliżej niż `OFFSHORE` kafli od brzegu kosztuje
//! więcej (`OFFSHORE_COST` za każdy kafel za blisko) – w wąskich przesmykach i tak przejdzie. Koszty liczy
//! Dijkstra z kubełkami (małe liczby całkowite) – jeden przebieg na port daje drogę do każdego brzegu; obok
//! kosztu pole trzyma prawdziwą długość drogi (czas rejsu, paliwo, zasięg portu). Pola są wyliczane
//! z niezmiennej mapy, więc trzymane poza hashem: kilka ostatnio używanych portów (`CACHE`).

use std::{cell::RefCell, rc::Rc};

use super::{Game, Water};

/// Długość kroku prosto i na skos (`STEP` = jeden kafel drogi).
pub const STEP: u32 = 5;
pub const DIAG: u32 = 7;
/// Droga omija pas morza przy lądzie: kafel w odległości `d` < `OFFSHORE` od brzegu kosztuje
/// dodatkowo (`OFFSHORE` − `d`) × `OFFSHORE_COST`.
pub const OFFSHORE: u32 = 4;
pub const OFFSHORE_COST: u32 = 3;
/// Najdroższy krok (kubełki Dijkstry).
const MAX_STEP: u32 = DIAG + OFFSHORE * OFFSHORE_COST;
/// Ile pól odległości (portów) pamiętać.
const CACHE: usize = 8;
/// Droga rejsu na mapie: co tyle kafli punkt (plus ostatni).
const PATH_EVERY: usize = 3;

/// Pole od portu (kafel lądu `origin`) do każdego kafla jego morza (`body`; indeks – numer kafla
/// w zbiorniku, `Water::local`): koszt drogi (`u32::MAX` – nieosiągalny) i jej długość (w `STEP`).
pub(super) struct SeaField {
    pub origin: u32,
    pub body: u32,
    pub cost: Vec<u32>,
    pub len: Vec<u32>,
}

/// Ostatnio używane pola odległości (najnowsze na końcu).
#[derive(Default)]
pub(super) struct SeaCache(RefCell<Vec<Rc<SeaField>>>);

const DIRS: [(i64, i64, u32); 8] = [
    (1, 0, STEP),
    (-1, 0, STEP),
    (0, 1, STEP),
    (0, -1, STEP),
    (1, 1, DIAG),
    (-1, 1, DIAG),
    (1, -1, DIAG),
    (-1, -1, DIAG),
];

impl Water {
    /// Odległość kafli wody od lądu (bokiem, najwyżej `OFFSHORE`; ląd – 0) – droga rejsu omija brzegi.
    pub(super) fn offshore(body: &[u32], w: usize, h: usize) -> Vec<u8> {
        let mut d = vec![OFFSHORE as u8; w * h];
        let mut front: Vec<usize> = (0..w * h).filter(|&i| body[i] == 0).collect();
        for &i in &front {
            d[i] = 0;
        }
        for step in 1..OFFSHORE as u8 {
            let mut next = Vec::new();
            for &i in &front {
                let (x, y) = (i % w, i / w);
                for (ok, j) in
                    [(x > 0, i.wrapping_sub(1)), (x + 1 < w, i + 1), (y > 0, i.wrapping_sub(w)), (y + 1 < h, i + w)]
                {
                    if ok && d[j] > step {
                        d[j] = step;
                        next.push(j);
                    }
                }
            }
            front = next;
        }
        d
    }

    /// Sąsiad kafla `i` w kierunku (dx, dy) w tym samym zbiorniku (na skos – oba boki też w zbiorniku).
    fn sea_step(&self, i: usize, dx: i64, dy: i64, w: usize, h: usize) -> Option<usize> {
        let b = self.body[i];
        let (x, y) = ((i % w) as i64 + dx, (i / w) as i64 + dy);
        if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
            return None;
        }
        let j = y as usize * w + x as usize;
        let side = |k: usize| self.body[k] == b;
        (side(j) && (dx == 0 || dy == 0 || (side(y as usize * w + i % w) && side((i / w) * w + x as usize))))
            .then_some(j)
    }

    /// Dodatkowy koszt wejścia na kafel blisko lądu.
    fn near_shore(&self, j: usize) -> u32 {
        (OFFSHORE - (self.shore[j] as u32).min(OFFSHORE)) * OFFSHORE_COST
    }

    /// Pole od portu na kaflu lądu `origin` (start z kafli morza stykających się z nim bokiem).
    fn sea_field(&self, origin: usize, w: usize, h: usize) -> Option<SeaField> {
        let body = self.sea_next_to(origin, w, h)?;
        let tiles = &self.tiles[body as usize - 1];
        let mut cost = vec![u32::MAX; tiles.len()];
        let mut len = vec![u32::MAX; tiles.len()];
        // Kubełki według kosztu modulo (najdroższy krok + 1).
        let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); MAX_STEP as usize + 1];
        let slots = buckets.len() as u32;
        let (x, y) = (origin % w, origin / w);
        let mut queued = 0usize;
        for (ok, j) in [
            (x > 0, origin.wrapping_sub(1)),
            (x + 1 < w, origin + 1),
            (y > 0, origin.wrapping_sub(w)),
            (y + 1 < h, origin + w),
        ] {
            if ok && self.body[j] == body && cost[self.local[j] as usize] != 0 {
                cost[self.local[j] as usize] = 0;
                len[self.local[j] as usize] = 0;
                buckets[0].push(j as u32);
                queued += 1;
            }
        }
        let mut d = 0u32;
        while queued > 0 {
            let slot = (d % slots) as usize;
            while let Some(i) = buckets[slot].pop() {
                queued -= 1;
                let i = i as usize;
                let ki = self.local[i] as usize;
                if cost[ki] != d {
                    continue;
                }
                for &(dx, dy, step) in &DIRS {
                    let Some(j) = self.sea_step(i, dx, dy, w, h) else { continue };
                    let k = self.local[j] as usize;
                    let c = d + step + self.near_shore(j);
                    if c < cost[k] {
                        cost[k] = c;
                        len[k] = len[ki] + step;
                        buckets[(c % slots) as usize].push(j as u32);
                        queued += 1;
                    }
                }
            }
            d += 1;
        }
        Some(SeaField { origin: origin as u32, body, cost, len })
    }

    /// Droga od portu do kafla morza `end` (kafle morza po kolei, co `PATH_EVERY` plus ostatni) – z pola,
    /// krok zawsze do sąsiada, z którego przyszła najtańsza droga.
    fn sea_path(&self, field: &SeaField, end: usize, w: usize, h: usize) -> Vec<u32> {
        let mut back = vec![end as u32];
        let mut i = end;
        let mut d = field.cost[self.local[end] as usize];
        while d > 0 && d != u32::MAX {
            let enter = self.near_shore(i);
            let Some((j, next)) = DIRS.iter().find_map(|&(dx, dy, step)| {
                let j = self.sea_step(i, dx, dy, w, h)?;
                let dj = field.cost[self.local[j] as usize];
                (dj != u32::MAX && dj + step + enter == d).then_some((j, dj))
            }) else {
                break;
            };
            back.push(j as u32);
            (i, d) = (j, next);
        }
        back.reverse();
        let last = back.len() - 1;
        back.into_iter().enumerate().filter(|&(k, _)| k % PATH_EVERY == 0 || k == last).map(|(_, t)| t).collect()
    }
}

impl Game {
    /// Pole drogi po morzu od portu na kaflu `origin` (z pamięci albo policzone i zapamiętane).
    pub(super) fn sea_field(&self, origin: usize) -> Option<Rc<SeaField>> {
        let mut cache = self.sea.0.borrow_mut();
        if let Some(k) = cache.iter().position(|f| f.origin as usize == origin) {
            let f = cache.remove(k);
            cache.push(f.clone());
            return Some(f);
        }
        let (w, h) = (self.map.width as usize, self.map.height as usize);
        let field = Rc::new(self.water.sea_field(origin, w, h)?);
        if cache.len() >= CACHE {
            cache.remove(0);
        }
        cache.push(field.clone());
        Some(field)
    }

    /// Długość drogi (kafle, `None` – nieosiągalny) od portu z pola `field` do kafla morza `sea`.
    pub(super) fn sea_distance(&self, field: &SeaField, sea: usize) -> Option<u32> {
        (self.water.body[sea] == field.body)
            .then(|| field.len[self.water.local[sea] as usize])
            .filter(|&d| d != u32::MAX)
            .map(|d| d.div_ceil(STEP) + 1)
    }

    /// Droga rejsu od portu `origin` do kafla morza `sea` (kafle co kilka).
    pub(super) fn sea_route(&self, origin: usize, sea: usize) -> Vec<u32> {
        let (w, h) = (self.map.width as usize, self.map.height as usize);
        self.sea_field(origin).map_or_else(Vec::new, |f| self.water.sea_path(&f, sea, w, h))
    }
}
