use std::collections::{HashMap, VecDeque, HashSet};
use std::io::{self, BufRead};

pub const PLAINS: i32 = 0;
pub const RIVER: i32 = 1;
pub const MOUNTAIN: i32 = 2;
pub const FREE: i32 = -1;
pub const NEUTRAL: i32 = 2;

#[derive(Clone, Debug)]
pub struct Cell {
    pub region_id: i32,
    pub cell_id: usize,
    pub terrain: i32,
    pub track_owner: i32,
    pub instability: i32,
    pub inked: bool,
    pub active: Vec<(i32, i32)>,
}

#[derive(Clone, Debug)]
pub struct Region {
    pub cell_ids: Vec<usize>,
    pub score: [i32; 2],
    pub has_town: bool,
}

impl Region {
    pub fn cells<'a>(&self, state: &'a State) -> impl Iterator<Item = &'a Cell> + use<'a, '_> {
        self.cell_ids.iter().map(|&i| &state.cells[i])
    }

    pub fn owner(&self) -> i32 {
        let [a, b] = self.score;
        if a == b {
            -1
        } else if a > b {
            0
        } else {
            1
        }
    }

    pub fn instability(&self, state: &State) -> i32 {
        state.cells[self.cell_ids[0]].instability
    }

    pub fn inked(&self, state: &State) -> bool {
        state.cells[self.cell_ids[0]].inked
    }
}

#[derive(Clone, Debug)]
pub struct Town {
    pub id: i32,
    pub x: usize,
    pub y: usize,
    pub desired: Vec<i32>,
}

#[derive(Clone, Debug)]
pub struct State {
    pub my_id: i32,
    pub foe_id: i32,
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
    pub towns: Vec<Town>,
    pub regions: HashMap<i32, Region>,
    pub connections: HashSet<(i32, i32)>,
    pub my_score: i32,
    pub foe_score: i32,
}

impl State {
    pub fn has_connection(&self, a: &Town, b: &Town) -> bool {
        self.connections.contains(&(a.id, b.id)) || self.connections.contains(&(b.id, a.id))
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    Place(usize, usize),
    Disrupt(usize),
    Auto(usize, usize, usize, usize),
    Wait,
}

impl Action {
    pub fn parse(s: &str) -> Result<Self, String> {
        let p: Vec<_> = s.split_whitespace().collect();
        if p.is_empty() {
            return Err("empty action".into());
        }
        match p[0] {
            "WAIT" if p.len() == 1 => Ok(Action::Wait),
            "PLACE_TRACKS" if p.len() == 3 => Ok(Action::Place(
                p[1].parse().map_err(|_| "bad x")?,
                p[2].parse().map_err(|_| "bad y")?,
            )),
            "AUTOPLACE" if p.len() == 5 => Ok(Action::Auto(
                p[1].parse().map_err(|_| "bad fromX")?,
                p[2].parse().map_err(|_| "bad fromY")?,
                p[3].parse().map_err(|_| "bad toX")?,
                p[4].parse().map_err(|_| "bad toY")?,
            )),
            "DISRUPT" if p.len() == 2 => Ok(Action::Disrupt(p[1].parse().map_err(|_| "bad id")?)),
            "MESSAGE" => {
                Err("MESSAGE is accepted by CodinGame but omitted from this local engine".into())
            }
            _ => Err(format!("bad action: {s}")),
        }
    }
}

pub fn idx(x: usize, y: usize, w: usize) -> usize {
    y * w + x
}

pub fn terrain_cost(t: i32) -> i32 {
    match t {
        PLAINS => 1,
        RIVER => 2,
        MOUNTAIN => 3,
        _ => 1,
    }
}

pub fn town_at(state: &State, x: usize, y: usize) -> Option<i32> {
    state
        .towns
        .iter()
        .find(|t| t.x == x && t.y == y)
        .map(|t| t.id)
}

pub fn is_rail_cell(state: &State, x: usize, y: usize) -> bool {
    town_at(state, x, y).is_some() || state.cells[idx(x, y, state.width)].track_owner != FREE
}

pub fn neighbors(x: usize, y: usize, w: usize, h: usize) -> Vec<(usize, usize)> {
    // Required tie-breaking order: NORTH, EAST, SOUTH, WEST.
    let mut out = Vec::with_capacity(4);
    if y > 0 {
        out.push((x, y - 1));
    }
    if x + 1 < w {
        out.push((x + 1, y));
    }
    if y + 1 < h {
        out.push((x, y + 1));
    }
    if x > 0 {
        out.push((x - 1, y));
    }
    out
}

pub fn cheapest_path(
    state: &State,
    from: (usize, usize),
    to: (usize, usize),
) -> Option<Vec<(usize, usize)>> {
    // Dijkstra on paint cost. Existing tracks and towns cost 0; empty cells
    // cost the terrain price. The priority queue is implemented simply with
    // repeated min selection because maps are only 21x14 .. 30x20.
    #[derive(Clone, Copy)]
    struct Node {
        cost: i32,
        x: usize,
        y: usize,
        seq: usize,
    }

    let n = state.width * state.height;
    let mut dist = vec![i32::MAX / 4; n];
    let mut parent: Vec<Option<usize>> = vec![None; n];
    let mut visited = vec![false; n];
    let mut seq = 0usize;
    let mut q = Vec::<Node>::new();

    let s = idx(from.0, from.1, state.width);
    let goal = idx(to.0, to.1, state.width);
    dist[s] = 0;
    q.push(Node {
        cost: 0,
        x: from.0,
        y: from.1,
        seq,
    });
    seq += 1;

    while !q.is_empty() {
        let mut best = 0usize;
        for i in 1..q.len() {
            if q[i].cost < q[best].cost || (q[i].cost == q[best].cost && q[i].seq < q[best].seq) {
                best = i;
            }
        }
        let node = q.swap_remove(best);
        let u = idx(node.x, node.y, state.width);
        if visited[u] {
            continue;
        }
        visited[u] = true;
        if u == goal {
            break;
        }

        for (nx, ny) in neighbors(node.x, node.y, state.width, state.height) {
            let v = idx(nx, ny, state.width);
            let extra = if is_rail_cell(state, nx, ny) {
                0
            } else {
                terrain_cost(state.cells[v].terrain)
            };
            let nd = node.cost + extra;
            if nd < dist[v] {
                dist[v] = nd;
                parent[v] = Some(u);
                q.push(Node {
                    cost: nd,
                    x: nx,
                    y: ny,
                    seq,
                });
                seq += 1;
            }
        }
    }

    if dist[goal] >= i32::MAX / 8 {
        return None;
    }
    let mut path = Vec::new();
    let mut cur = goal;
    path.push((cur % state.width, cur / state.width));
    while cur != s {
        cur = parent[cur]?;
        path.push((cur % state.width, cur / state.width));
    }
    path.reverse();
    Some(path)
}

pub fn shortest_active_path(
    state: &State,
    from: (usize, usize),
    to: (usize, usize),
) -> Option<Vec<(usize, usize)>> {
    // BFS on towns + track cells. Neighbor order implements N,E,S,W tie-break.
    let n = state.width * state.height;
    let s = idx(from.0, from.1, state.width);
    let goal = idx(to.0, to.1, state.width);
    let mut seen = vec![false; n];
    let mut parent = vec![None; n];
    let mut q = VecDeque::new();
    seen[s] = true;
    q.push_back(s);

    while let Some(u) = q.pop_front() {
        if u == goal {
            break;
        }
        let x = u % state.width;
        let y = u / state.width;
        for (nx, ny) in neighbors(x, y, state.width, state.height) {
            let v = idx(nx, ny, state.width);
            if !seen[v] && is_rail_cell(state, nx, ny) {
                seen[v] = true;
                parent[v] = Some(u);
                q.push_back(v);
            }
        }
    }

    if !seen[goal] {
        return None;
    }
    let mut path = Vec::new();
    let mut cur = goal;
    path.push((cur % state.width, cur / state.width));
    while cur != s {
        cur = parent[cur]?;
        path.push((cur % state.width, cur / state.width));
    }
    path.reverse();
    Some(path)
}

pub fn recompute_active(state: &mut State) {
    for c in &mut state.cells {
        c.active.clear();
    }

    // Each unilateral desired connection is an independent connection.
    for a in &state.towns {
        for &b_id in &a.desired {
            if let Some(b) = state.towns.iter().find(|t| t.id == b_id) {
                if let Some(path) = shortest_active_path(state, (a.x, a.y), (b.x, b.y)) {
                    for &(x, y) in &path {
                        state.cells[idx(x, y, state.width)]
                            .active
                            .push((a.id, b.id));
                    }
                }
            }
        }
    }
}

pub fn score_connections(state: &State) -> (i32, i32) {
    let mut a = 0;
    let mut b = 0;
    for town in &state.towns {
        for &other_id in &town.desired {
            let other = match state.towns.iter().find(|t| t.id == other_id) {
                Some(t) => t,
                None => continue,
            };
            let path = match shortest_active_path(state, (town.x, town.y), (other.x, other.y)) {
                Some(p) => p,
                None => continue,
            };
            for (x, y) in path {
                match state.cells[idx(x, y, state.width)].track_owner {
                    0 => a += 1,
                    1 => b += 1,
                    _ => {}
                }
            }
        }
    }
    (a, b)
}

pub fn any_active_connection(state: &State) -> bool {
    for town in &state.towns {
        for &other_id in &town.desired {
            if let Some(other) = state.towns.iter().find(|t| t.id == other_id) {
                if shortest_active_path(state, (town.x, town.y), (other.x, other.y)).is_some() {
                    return true;
                }
            }
        }
    }
    false
}

pub fn parse_initial<R: BufRead>(r: &mut R) -> io::Result<State> {
    let mut line = String::new();
    r.read_line(&mut line)?;
    let my_id = line.trim().parse().unwrap();
    let foe_id = (my_id + 1) % 2;
    line.clear();
    r.read_line(&mut line)?;
    let width: usize = line.trim().parse().unwrap();
    line.clear();
    r.read_line(&mut line)?;
    let height: usize = line.trim().parse().unwrap();

    let mut regions = HashMap::new();

    let mut cells = Vec::with_capacity(width * height);
    for i in 0..width * height {
        line.clear();
        r.read_line(&mut line)?;
        let p: Vec<_> = line.split_whitespace().collect();
        let region_id = p[0].parse().unwrap();
        cells.push(Cell {
            region_id,
            cell_id: i,
            terrain: p[1].parse().unwrap(),
            track_owner: FREE,
            instability: 0,
            inked: false,
            active: Vec::new(),
        });
        let region = regions.entry(region_id).or_insert(Region {
            cell_ids: Vec::new(),
            score: [0, 0],
            has_town: false,
        });
        region.cell_ids.push(i);
    }

    line.clear();
    r.read_line(&mut line)?;
    let town_count: usize = line.trim().parse().unwrap();
    let mut towns = Vec::with_capacity(town_count);
    for _ in 0..town_count {
        line.clear();
        r.read_line(&mut line)?;
        let p: Vec<_> = line.split_whitespace().collect();
        let desired = if p[3] == "x" {
            vec![]
        } else {
            p[3].split(',').map(|x| x.parse().unwrap()).collect()
        };
        let (x, y) = (p[1].parse().unwrap(), p[2].parse().unwrap());
        towns.push(Town {
            id: p[0].parse().unwrap(),
            x,
            y,
            desired,
        });
        let region = regions.get_mut(&cells[x + y * width].region_id);
        if let Some(r) = region {
            r.has_town = true;
        } else {
            panic!("region not found");
        }

    }

    Ok(State {
        my_id,
        foe_id,
        width,
        height,
        cells,
        towns,
        regions,
        connections: HashSet::new(),
        my_score: 0,
        foe_score: 0,
    })
}

pub fn read_turn<R: BufRead>(r: &mut R, state: &mut State) -> io::Result<()> {
    let mut line = String::new();
    line.clear();
    if r.read_line(&mut line)? == 0 {
        return Ok(());
    }
    state.my_score = line.trim().parse().unwrap();
    line.clear();
    r.read_line(&mut line)?;
    state.foe_score = line.trim().parse().unwrap();

    for (key, value) in &mut state.regions {
        value.score = [0, 0];
    }
    state.connections = HashSet::new();

    for i in 0..state.width * state.height {
        line.clear();
        r.read_line(&mut line)?;
        let p: Vec<_> = line.split_whitespace().collect();
        let track_owner = p[0].parse().unwrap();
        state.cells[i].track_owner = track_owner;

        state.cells[i].instability = p[1].parse().unwrap();
        state.cells[i].inked = p[2].parse::<i32>().unwrap() != 0;
        state.cells[i].active.clear();
        if p.len() > 3 && p[3] != "x" {
            for pair in p[3].split(',') {
                let q: Vec<_> = pair.split('-').collect();
                if q.len() == 2 {
                    if let (Ok(a), Ok(b)) = (q[0].parse(), q[1].parse()) {
                        state.cells[i].active.push((a, b));
                        state.connections.insert((a, b));
                    }
                }
            }
        }
        let region = state.regions.get_mut(&state.cells[i].region_id);
        if let Some(r) = region {
            if track_owner != -1 && track_owner != 2 && !state.cells[i].inked {
                r.score[track_owner as usize] +=
                    (1 + 2 * (state.cells[i].active.len() as i32) + state.cells[i].instability);
            }
        } else {
            panic!("region not found");
        }
        // if !state.cells[i].active.is_empty() {
        //     eprintln!("cell {:?}", state.cells[i]);
        // }
        // for (key, value) in &state.regions {
        //     if value.owner() == 0 || value.owner() == 1 {
        //         eprintln!("score (region {key}): {0:?}", { value.score });
        //     }
        // }
    }
    Ok(())
}



#[derive(Clone, Debug)]
pub struct Connection {
    pub id: (i32, i32),
    pub path: Vec<(usize, usize)>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Direction {
    North,
    East,
    South,
    West,
}

impl Direction {
    fn priority() -> [Direction; 4] {
        [
            Direction::North,
            Direction::East,
            Direction::South,
            Direction::West,
        ]
    }

    fn rank(self) -> usize {
        match self {
            Direction::North => 0,
            Direction::East => 1,
            Direction::South => 2,
            Direction::West => 3,
        }
    }
}

/// Returns the direction between two adjacent coordinates.
///
/// Y increases downward:
///     North = y - 1
///     South = y + 1
fn direction_between(
    from: (usize, usize),
    to: (usize, usize),
) -> Direction {
    match (
        to.0 as isize - from.0 as isize,
        to.1 as isize - from.1 as isize,
    ) {
        (0, -1) => Direction::North,
        (1, 0) => Direction::East,
        (0, 1) => Direction::South,
        (-1, 0) => Direction::West,
        _ => panic!("Coordinates are not adjacent"),
    }
}

/// Builds the lexicographically best Manhattan shortest path
/// from `origin` to `desired`.
///
fn best_forward_path(
    origin: (usize, usize),
    desired: (usize, usize),
) -> Vec<(usize, usize)> {
    let dx = desired.0 as isize - origin.0 as isize;
    let dy = desired.1 as isize - origin.1 as isize;

    let horizontal = dx.unsigned_abs();
    let vertical = dy.unsigned_abs();

    // We don't include either town, so the number of coordinates
    // is Manhattan distance - 1.
    let path_len = horizontal + vertical - 1;

    let mut path = Vec::with_capacity(path_len);

    let mut x = origin.0;
    let mut y = origin.1;


    // NORTH
    if dy < 0 {
        for _ in 0..vertical {
            y -= 1;

            if (x, y) != desired {
                path.push((x, y));
            }
        }
    }

    // EAST
    if dx > 0 {
        for _ in 0..horizontal {
            x += 1;

            if (x, y) != desired {
                path.push((x, y));
            }
        }
    }

    // SOUTH
    if dy > 0 {
        for _ in 0..vertical {
            y += 1;

            if (x, y) != desired {
                path.push((x, y));
            }
        }
    }

    // WEST
    if dx < 0 {
        for _ in 0..horizontal {
            x -= 1;

            if (x, y) != desired {
                path.push((x, y));
            }
        }
    }

    path
}

/// Find all requested town connections.
///
pub fn find_connections(state: &State) -> Vec<Connection> {
    // -------------------------------------------------------------
    // Build an ID -> coordinate map.
    //
    // We only need coordinates while constructing connections, so
    // storing references isn't necessary.
    // -------------------------------------------------------------
    let towns: HashMap<i32, (usize, usize)> = state
        .towns
        .iter()
        .map(|town| (town.id, (town.x, town.y)))
        .collect();

    // Count connections first so we can allocate the result once.
    let connection_count: usize = state
        .towns
        .iter()
        .map(|town| town.desired.len())
        .sum();

    let mut connections = Vec::with_capacity(connection_count);

    for origin_town in &state.towns {
        let origin = (origin_town.x, origin_town.y);

        for &desired_id in &origin_town.desired {
            let desired = towns[&desired_id];

            // -----------------------------------------------------
            // STEP 1:
            //
            // Find the best shortest path when travelling
            // origin -> desired.
            // -----------------------------------------------------
            let forward_path = best_forward_path(origin, desired);

            let forward_first_step = forward_path[0];

            let reverse_first_step = if forward_path.len() == 1 {
                // This case is actually impossible under the
                // "no contiguous towns" assumption, but keeping
                // it makes the function robust.
                origin
            } else {
                forward_path[forward_path.len() - 1]
            };

            let forward_direction =
                direction_between(origin, forward_first_step);

            let reverse_direction =
                direction_between(desired, reverse_first_step);

            // -----------------------------------------------------
            // STEP 2:
            //
            // Choose orientation by FIRST STEP:
            //
            // NORTH > EAST > SOUTH > WEST
            // -----------------------------------------------------
            let path = if forward_direction.rank()
                <= reverse_direction.rank()
            {
                forward_path
            } else {
                forward_path.into_iter().rev().collect()
            };

            connections.push(Connection {
                id: (origin_town.id, desired_id),
                path,
            });
        }
    }

    connections
}
