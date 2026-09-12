use std::collections::{HashMap, VecDeque};
use std::io::{self, BufRead};

pub const PLAINS: i32 = 0;
pub const RIVER: i32 = 1;
pub const MOUNTAIN: i32 = 2;
pub const FREE: i32 = -1;
pub const NEUTRAL: i32 = 2;

#[derive(Clone, Debug)]
pub struct Cell {
    pub region_id: i32,
    pub terrain: i32,
    pub track_owner: i32,
    pub instability: i32,
    pub inked: bool,
    pub active: Vec<(i32, i32)>,
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
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
    pub towns: Vec<Town>,
    pub my_score: i32,
    pub foe_score: i32,
}

#[derive(Clone, Debug)]
pub enum Action {
    Place(usize, usize),
    Auto(usize, usize, usize, usize),
    Wait,
}

impl Action {
    pub fn parse(s: &str) -> Result<Self, String> {
        let p: Vec<_> = s.split_whitespace().collect();
        if p.is_empty() { return Err("empty action".into()); }
        match p[0] {
            "WAIT" if p.len() == 1 => Ok(Action::Wait),
            "PLACE_TRACKS" if p.len() == 3 => {
                Ok(Action::Place(
                    p[1].parse().map_err(|_| "bad x")?,
                    p[2].parse().map_err(|_| "bad y")?,
                ))
            }
            "AUTOPLACE" if p.len() == 5 => {
                Ok(Action::Auto(
                    p[1].parse().map_err(|_| "bad fromX")?,
                    p[2].parse().map_err(|_| "bad fromY")?,
                    p[3].parse().map_err(|_| "bad toX")?,
                    p[4].parse().map_err(|_| "bad toY")?,
                ))
            }
            "MESSAGE" => Err("MESSAGE is accepted by CodinGame but omitted from this local engine".into()),
            _ => Err(format!("bad action: {s}")),
        }
    }
}

pub fn idx(x: usize, y: usize, w: usize) -> usize { y * w + x }

pub fn terrain_cost(t: i32) -> i32 {
    match t {
        PLAINS => 1,
        RIVER => 2,
        MOUNTAIN => 3,
        _ => 1,
    }
}

pub fn town_at(state: &State, x: usize, y: usize) -> Option<i32> {
    state.towns.iter().find(|t| t.x == x && t.y == y).map(|t| t.id)
}

pub fn is_rail_cell(state: &State, x: usize, y: usize) -> bool {
    town_at(state, x, y).is_some() || state.cells[idx(x,y,state.width)].track_owner != FREE
}

pub fn neighbors(x: usize, y: usize, w: usize, h: usize) -> Vec<(usize, usize)> {
    // Required tie-breaking order: NORTH, EAST, SOUTH, WEST.
    let mut out = Vec::with_capacity(4);
    if y > 0 { out.push((x, y - 1)); }
    if x + 1 < w { out.push((x + 1, y)); }
    if y + 1 < h { out.push((x, y + 1)); }
    if x > 0 { out.push((x - 1, y)); }
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
    struct Node { cost: i32, x: usize, y: usize, seq: usize }

    let n = state.width * state.height;
    let mut dist = vec![i32::MAX / 4; n];
    let mut parent: Vec<Option<usize>> = vec![None; n];
    let mut visited = vec![false; n];
    let mut seq = 0usize;
    let mut q = Vec::<Node>::new();

    let s = idx(from.0, from.1, state.width);
    let goal = idx(to.0, to.1, state.width);
    dist[s] = 0;
    q.push(Node { cost: 0, x: from.0, y: from.1, seq });
    seq += 1;

    while !q.is_empty() {
        let mut best = 0usize;
        for i in 1..q.len() {
            if q[i].cost < q[best].cost ||
               (q[i].cost == q[best].cost && q[i].seq < q[best].seq) {
                best = i;
            }
        }
        let node = q.swap_remove(best);
        let u = idx(node.x, node.y, state.width);
        if visited[u] { continue; }
        visited[u] = true;
        if u == goal { break; }

        for (nx, ny) in neighbors(node.x, node.y, state.width, state.height) {
            let v = idx(nx, ny, state.width);
            let extra = if is_rail_cell(state, nx, ny) { 0 } else {
                terrain_cost(state.cells[v].terrain)
            };
            let nd = node.cost + extra;
            if nd < dist[v] {
                dist[v] = nd;
                parent[v] = Some(u);
                q.push(Node { cost: nd, x: nx, y: ny, seq });
                seq += 1;
            }
        }
    }

    if dist[goal] >= i32::MAX / 8 { return None; }
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
        if u == goal { break; }
        let x = u % state.width;
        let y = u / state.width;
        for (nx, ny) in neighbors(x,y,state.width,state.height) {
            let v = idx(nx,ny,state.width);
            if !seen[v] && is_rail_cell(state,nx,ny) {
                seen[v] = true;
                parent[v] = Some(u);
                q.push_back(v);
            }
        }
    }

    if !seen[goal] { return None; }
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
    for c in &mut state.cells { c.active.clear(); }

    // Each unilateral desired connection is an independent connection.
    for a in &state.towns {
        for &b_id in &a.desired {
            if let Some(b) = state.towns.iter().find(|t| t.id == b_id) {
                if let Some(path) = shortest_active_path(state, (a.x,a.y), (b.x,b.y)) {
                    for &(x,y) in &path {
                        state.cells[idx(x,y,state.width)].active.push((a.id,b.id));
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
            let path = match shortest_active_path(state,(town.x,town.y),(other.x,other.y)) {
                Some(p) => p,
                None => continue,
            };
            for (x,y) in path {
                match state.cells[idx(x,y,state.width)].track_owner {
                    0 => a += 1,
                    1 => b += 1,
                    _ => {}
                }
            }
        }
    }
    (a,b)
}

pub fn any_active_connection(state: &State) -> bool {
    for town in &state.towns {
        for &other_id in &town.desired {
            if let Some(other) = state.towns.iter().find(|t| t.id == other_id) {
                if shortest_active_path(state,(town.x,town.y),(other.x,other.y)).is_some() {
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
    line.clear(); r.read_line(&mut line)?;
    let width: usize = line.trim().parse().unwrap();
    line.clear(); r.read_line(&mut line)?;
    let height: usize = line.trim().parse().unwrap();

    let mut cells = Vec::with_capacity(width*height);
    for _ in 0..width*height {
        line.clear(); r.read_line(&mut line)?;
        let p: Vec<_> = line.split_whitespace().collect();
        cells.push(Cell {
            region_id: p[0].parse().unwrap(),
            terrain: p[1].parse().unwrap(),
            track_owner: FREE,
            instability: 0,
            inked: false,
            active: Vec::new(),
        });
    }

    line.clear(); r.read_line(&mut line)?;
    let town_count: usize = line.trim().parse().unwrap();
    let mut towns = Vec::with_capacity(town_count);
    for _ in 0..town_count {
        line.clear(); r.read_line(&mut line)?;
        let p: Vec<_> = line.split_whitespace().collect();
        let desired = if p[3] == "x" { vec![] } else {
            p[3].split(',').map(|x| x.parse().unwrap()).collect()
        };
        towns.push(Town {
            id: p[0].parse().unwrap(),
            x: p[1].parse().unwrap(),
            y: p[2].parse().unwrap(),
            desired,
        });
    }

    Ok(State {
        my_id, width, height, cells, towns, my_score: 0, foe_score: 0
    })
}

pub fn read_turn<R: BufRead>(r: &mut R, state: &mut State) -> io::Result<()> {
    let mut line = String::new();
    line.clear();
    if r.read_line(&mut line)? == 0 { return Ok(()); }
    state.my_score = line.trim().parse().unwrap();
    line.clear(); r.read_line(&mut line)?;
    state.foe_score = line.trim().parse().unwrap();

    for i in 0..state.width*state.height {
        line.clear(); r.read_line(&mut line)?;
        let p: Vec<_> = line.split_whitespace().collect();
        state.cells[i].track_owner = p[0].parse().unwrap();
        state.cells[i].instability = p[1].parse().unwrap();
        state.cells[i].inked = p[2].parse::<i32>().unwrap() != 0;
        state.cells[i].active.clear();
        if p.len() > 3 && p[3] != "x" {
            for pair in p[3].split(',') {
                let q: Vec<_> = pair.split('-').collect();
                if q.len() == 2 {
                    if let (Ok(a),Ok(b)) = (q[0].parse(),q[1].parse()) {
                        state.cells[i].active.push((a,b));
                    }
                }
            }
        }
    }
    Ok(())
}
