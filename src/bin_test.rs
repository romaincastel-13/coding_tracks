mod common;
use std::collections::{HashMap, VecDeque, HashSet};

use crate::common::*;

fn main() {

    let mut towns = Vec::new();
    towns.push(
        Town {
            id: towns.len(),
            x : 6,
            y: 0,
            desired: vec![1]
        }
    );
    towns.push(
        Town {
            id: towns.len(),
            x : 3,
            y: 3,
            desired: Vec::new()
        }
    );

    let mut state = make_map(5, 30, 30);
    state.cells[idx(10, 4, state.width)].inked = true;

    let res2 = find_best_initial_connections(&state);
    let res3 = bfs_that_returns_all_shortest(&state, (10,5), (3, 3));
    let res = choose_best_among_shortest_paths(res3);
    println!("{res:?}");

}

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, n: usize) -> usize {
        (self.next() as usize) % n
    }
}

fn make_map(seed: u64, width:usize, height:usize) -> State {
    let mut rng = Rng::new(seed);
    let mut cells = Vec::with_capacity(width * height);
    let mut i = 0;

    // Simple deterministic regions: 3x2-ish blocks, with random terrain.
    for y in 0..height {
        for x in 0..width {
            let region_id = ((x / 3) + (y / 3) * ((width + 2) / 3)) as i32;
            let terrain = rng.range(100);
            let terrain = if terrain < 70 {
                PLAINS
            } else if terrain < 90 {
                RIVER
            } else {
                MOUNTAIN
            };
            cells.push(Cell {
                region_id,
                cell_id: i,
                terrain,
                track_owner: FREE,
                instability: 0,
                inked: false,
                active: vec![],
                town_id: None
            });
            i += 1;
        }
    }

    // Pick towns with no two towns adjacent and only on plains.
    let mut positions = Vec::<(usize, usize)>::new();
    let target = 6;
    let mut tries = 0;
    while positions.len() < target && tries < 10000 {
        tries += 1;
        let x = rng.range(width);
        let y = rng.range(height);
        if cells[idx(x, y, width)].terrain != PLAINS {
            continue;
        }
        if positions.iter().any(|&(px, py)| {
            let dx = px.abs_diff(x);
            let dy = py.abs_diff(y);
            dx <= 1 && dy <= 1
        }) {
            continue;
        }
        // one town per region
        let region = cells[idx(x, y, width)].region_id;
        if positions
            .iter()
            .any(|&(px, py)| cells[idx(px, py, width)].region_id == region)
        {
            continue;
        }
        positions.push((x, y));
    }

    let mut towns = Vec::new();
    for (i, (x, y)) in positions.iter().enumerate() {
        towns.push(Town {
            id: i,
            x: *x,
            y: *y,
            desired: vec![],
        });

    }

    // Guarantee a simple objective: town 0 wants town 1.
    if towns.len() >= 2 {
        towns[0].desired.push(1);
    }

    // Add a few more unilateral desires.
    for i in 1..towns.len() {
        if towns.len() > 3 && rng.range(3) == 0 {
            let j = rng.range(towns.len());
            if j != i && !towns[i].desired.contains(&j) {
                towns[i].desired.push(j);
            }
        }
    }

    State {
        my_id: 0,
        foe_id: 1,
        width: width,
        height: height,
        cells,
        towns,
        connections: HashMap::new(),
        regions: HashMap::new(),
        my_score: 0,
        foe_score: 0,
        top_cells: [0; 3]
    }
}