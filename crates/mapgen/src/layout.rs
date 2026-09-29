//! Makro-układ mapy: siatka chunków, z których każdy należy do kontynentu albo jest wodny.
//! Kontynenty rosną z rozstawionych centrów i nigdy nie stykają się ze sobą
//! (nawet po przekątnej), więc między nimi zawsze jest co najmniej jeden chunk wody.

use crate::{util::Rng, MapGenParams};

pub struct Layout {
    pub cols: usize,
    pub rows: usize,
    /// -1 = chunk wodny, inaczej indeks kontynentu.
    pub owner: Vec<i32>,
    pub width: usize,
    pub height: usize,
}

impl Layout {
    pub fn chunk_of(&self, x: usize, y: usize) -> usize {
        let cx = x * self.cols / self.width;
        let cy = y * self.rows / self.height;
        cy * self.cols + cx
    }
    pub fn is_water_chunk_at(&self, x: usize, y: usize) -> bool {
        self.owner[self.chunk_of(x, y)] < 0
    }
    /// Typowy rozmiar chunka w kaflach (krótszy bok).
    pub fn chunk_size(&self) -> f32 {
        (self.width as f32 / self.cols as f32).min(self.height as f32 / self.rows as f32)
    }
}

pub fn build(p: &MapGenParams, rng: &mut Rng) -> Layout {
    let (cols, rows) = (p.chunk_cols as usize, p.chunk_rows as usize);
    let total = cols * rows;
    let target_land = ((p.land_ratio * total as f32).round() as usize).clamp(1, total);
    let mut owner = vec![-1i32; total];

    let cheb = |a: usize, b: usize| {
        let (ax, ay, bx, by) = (a % cols, a / cols, b % cols, b / cols);
        ax.abs_diff(bx).max(ay.abs_diff(by))
    };

    // 1. Centra kontynentów: losowe pierwsze, kolejne jak najdalej od poprzednich.
    let wanted = (p.continents as usize).min(target_land);
    let mut centers = vec![rng.below(total)];
    while centers.len() < wanted {
        let score = |c: usize| centers.iter().map(|&o| cheb(c, o)).min().unwrap_or(usize::MAX);
        let best = (0..total).map(score).max().unwrap_or(0);
        if best < 2 {
            break; // brak miejsca na kolejny kontynent z przerwą wodną
        }
        let candidates: Vec<usize> = (0..total).filter(|&c| score(c) == best).collect();
        centers.push(candidates[rng.below(candidates.len())]);
    }
    let k = centers.len();
    for (i, &c) in centers.iter().enumerate() {
        owner[c] = i as i32;
    }

    // 2. Docelowe rozmiary: losowe wagi, im większe size_variance, tym większe różnice.
    let weights: Vec<f32> = (0..k)
        .map(|_| 1.0 + p.size_variance * 0.9 * (rng.f32() * 2.0 - 1.0))
        .collect();
    let wsum: f32 = weights.iter().sum();
    let mut targets: Vec<usize> = weights
        .iter()
        .map(|w| ((target_land as f32 * w / wsum).round() as usize).max(1))
        .collect();
    let mut sizes = vec![1usize; k];
    let mut land = k;

    let neighbors8 = |c: usize| {
        let (cx, cy) = ((c % cols) as i32, (c / cols) as i32);
        let mut out = Vec::with_capacity(8);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (nx, ny) = (cx + dx, cy + dy);
                if (dx, dy) != (0, 0) && nx >= 0 && ny >= 0 && (nx as usize) < cols && (ny as usize) < rows {
                    out.push(ny as usize * cols + nx as usize);
                }
            }
        }
        out
    };

    // 3. Wzrost po kolei (round-robin). Druga faza znosi limity, żeby dobić do target_land.
    for phase in 0..2 {
        if phase == 1 {
            targets.iter_mut().for_each(|t| *t = usize::MAX);
        }
        loop {
            let mut grew = false;
            for i in 0..k {
                if land >= target_land || sizes[i] >= targets[i] {
                    continue;
                }
                let mut best: Option<(f32, usize)> = None;
                for c in 0..total {
                    if owner[c] != -1 {
                        continue;
                    }
                    let (cx, cy) = (c % cols, c / cols);
                    let n8 = neighbors8(c);
                    let touches_self_4 = n8.iter().any(|&n| {
                        owner[n] == i as i32 && ((n % cols) == cx || (n / cols) == cy)
                    });
                    let touches_other = n8.iter().any(|&n| owner[n] >= 0 && owner[n] != i as i32);
                    if !touches_self_4 || touches_other {
                        continue;
                    }
                    // Zwarte kształty: preferuj chunki z wieloma sąsiadami z tego samego kontynentu.
                    let same = n8.iter().filter(|&&n| owner[n] == i as i32).count() as f32;
                    let score = same + rng.f32() * 2.5;
                    if best.is_none_or(|(s, _)| score > s) {
                        best = Some((score, c));
                    }
                }
                if let Some((_, c)) = best {
                    owner[c] = i as i32;
                    sizes[i] += 1;
                    land += 1;
                    grew = true;
                }
            }
            if !grew || land >= target_land {
                break;
            }
        }
    }

    Layout { cols, rows, owner, width: p.width as usize, height: p.height as usize }
}
