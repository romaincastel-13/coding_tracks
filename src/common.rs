use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::io::{self, BufRead};

use eframe::wgpu::wgc::device::UserClosures;
use egui::widget_style;

//devrait distinguer best path / cheapest path (for instability d4une connection , etc ?) ?

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
    pub active: Vec<(usize, usize)>,
    pub town_id: Option<usize>,
}

impl Cell {
    pub fn score(&self, state: &State, player: usize) -> i32 {
        if self.town_id != None || self.track_owner != -1 || self.inked {
            return -1000000;
        }
        let mut score = 0;
        let cell_coord = (self.cell_id % state.width, self.cell_id / state.width);
        for (_, connection) in &state.connections {
            for coord in &connection.best_path {
                if *coord == cell_coord {
                    score += 1;
                }
            }
            for coord in &connection.quickest_path {
                if *coord == cell_coord {
                    score += 1;
                }
            }
        }
        score
    }
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
    pub id: usize,
    pub x: usize,
    pub y: usize,
    pub desired: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct Connection {
    id: (usize, usize),
    active_path: Vec<(usize, usize)>,
    best_path: Vec<(usize, usize)>,
    quickest_path: Vec<(usize, usize)>,
    correlation: usize,
    ownership: [usize; 2],
    paints_to_activate: usize,
    dead: bool,
    instability: i32,
    most_instable: (usize, usize),
}

impl Connection {
    pub fn new(id: (usize, usize), dead: bool) -> Connection {
        Connection {
            id,
            active_path: Vec::new(),
            best_path: Vec::new(),
            quickest_path: Vec::new(),
            correlation: 0,
            ownership: [0, 0],
            paints_to_activate: 0,
            dead,
            instability: -1,
            most_instable: (0, 0),
        }
    }

    pub fn score(&self, state: &State, player: usize) -> i32 {
        let mut connection_score  = 0;
        let mult = if self.ownership[player] >= self.ownership[(player + 1) % 2] - 1 {
            3
        } else {
            1
        };

        connection_score -= self.instability;
        connection_score += self.best_path.len() as i32;
        connection_score -= self.paints_to_activate as i32 * 2;
        connection_score *= mult;
        connection_score
    }

    pub fn analyse(&mut self, state: &State) {
        if self.dead {
            return;
        }
        if !self.active_path.is_empty() {
            let mut p0 = 0;
            let mut p1 = 0;
            let l = self.active_path.len();
            for (x, y) in &self.active_path {
                let c = &state.cells[idx(*x, *y, state.width)];
                if c.track_owner == 0 || c.track_owner == 2 {
                    p0 += 1;
                }
                if c.track_owner == 1 || c.track_owner == 2 {
                    p1 += 1;
                }
            }
            self.ownership = [p0 * 100 / l, p1 * 100 / l];
            return;
        }
        let (from, to) = (
            (state.towns[self.id.0].x, state.towns[self.id.0].y),
            (state.towns[self.id.1].x, state.towns[self.id.1].y),
        );
        self.best_path = bfs_shortest(state, from, to);

        if self.best_path.is_empty() {
            self.dead = true;
            return;
        }
        if let Some(p) = cheapest_path(state, from, to) {
            self.paints_to_activate = p.len();
            self.quickest_path = p;
        } else {
            panic!("cheapest_path not found?? bug??");
        }
        for (x, y) in &self.best_path {
            let i = state.cells[idx(*x, *y, state.width)].instability;
            if i > self.instability {
                self.instability = i;
                self.most_instable = (*x, *y);
            }
        }
        let set: HashSet<_> = self.best_path.iter().collect();
        let similitudes = self
            .quickest_path
            .iter()
            .filter(|x| set.contains(x))
            .count();
        self.correlation = similitudes * 100 / self.best_path.len();
        let mut p0 = 0;
        let mut p1 = 0;
        let l = self.quickest_path.len() + self.best_path.len();
        for (x, y) in &self.best_path {
            let c = &state.cells[idx(*x, *y, state.width)];
            if c.track_owner == 0 || c.track_owner == 2 {
                p0 += 1;
            }
            if c.track_owner == 1 || c.track_owner == 2 {
                p1 += 1;
            }
        }
        for (x, y) in &self.quickest_path {
            let c = &state.cells[idx(*x, *y, state.width)];
            if c.track_owner == 0 || c.track_owner == 2 {
                p0 += 1;
            }
            if c.track_owner == 1 || c.track_owner == 2 {
                p1 += 1;
            }
            // let i = c.instability;
            // if i > self.instability {
            //     self.instability = i;
            //     self.most_instable = (*x, *y);
            // }
        }
        self.ownership = [p0 * 100 / l, p1 * 100 / l];
    }
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
    pub connections: HashMap<(usize, usize), Connection>,
    pub my_score: i32,
    pub foe_score: i32,
    pub top_cells: [usize; 3],
}

impl State {
    pub fn has_connection(&self, a: &Town, b: &Town) -> bool {
        self.connections.contains_key(&(a.id, b.id)) || self.connections.contains_key(&(b.id, a.id))
    }

    pub fn analyse_connections(&mut self) {
        let mut connections = std::mem::take(&mut self.connections);

        for connection in connections.values_mut() {
            connection.analyse(self);
        }

        self.connections = connections;
    }

    pub fn score_cells(&mut self, player: usize) {
        let mut top_scores = [-10000; 3];
        self.top_cells = [0; 3];
        for cell in &self.cells {
            let score = cell.score(self, player);
            if score > top_scores[0] {
                top_scores[0] = score;
                self.top_cells[0] = cell.cell_id;
                if top_scores[0] > top_scores[1] {
                    top_scores.swap(0, 1);
                    self.top_cells.swap(0, 1);
                }
                if top_scores[1] > top_scores[2] {
                    top_scores.swap(1, 2);
                    self.top_cells.swap(1, 2);
                }
            }
        }
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

pub fn bfs_shortest(
    state: &State,
    from: (usize, usize),
    to: (usize, usize),
) -> Vec<(usize, usize)> {
    let width = state.width;
    let height = state.height;
    let size = width * height;

    let start = idx(from.0, from.1, width);
    let target = idx(to.0, to.1, width);

    // usize::MAX means "not visited"
    let mut dist = vec![usize::MAX; size];

    // Store only ONE parent per cell.
    let mut parent = vec![usize::MAX; size];

    let mut frontier = VecDeque::new();

    dist[start] = 0;
    frontier.push_back(start);

    while let Some(current) = frontier.pop_front() {
        if current == target {
            break;
        }

        let x = current % width;
        let y = current / width;
        let next_dist = dist[current] + 1;

        // NORTH
        if y > 0 {
            visit(
                x,
                y - 1,
                current,
                next_dist,
                state,
                width,
                &mut dist,
                &mut parent,
                &mut frontier,
            );
        }

        // EAST
        if x + 1 < width {
            visit(
                x + 1,
                y,
                current,
                next_dist,
                state,
                width,
                &mut dist,
                &mut parent,
                &mut frontier,
            );
        }

        // SOUTH
        if y + 1 < height {
            visit(
                x,
                y + 1,
                current,
                next_dist,
                state,
                width,
                &mut dist,
                &mut parent,
                &mut frontier,
            );
        }

        // WEST
        if x > 0 {
            visit(
                x - 1,
                y,
                current,
                next_dist,
                state,
                width,
                &mut dist,
                &mut parent,
                &mut frontier,
            );
        }
    }

    // No path.
    if dist[target] == usize::MAX {
        return Vec::new();
    }

    // Reconstruct path backwards.
    let mut path = Vec::with_capacity(dist[target] + 1);

    let mut current = target;

    loop {
        let x = current % width;
        let y = current / width;

        path.push((x, y));

        if current == start {
            break;
        }

        current = parent[current];
    }

    path.reverse();
    path
}

#[inline]
fn visit(
    x: usize,
    y: usize,
    current: usize,
    next_dist: usize,
    state: &State,
    width: usize,
    dist: &mut [usize],
    parent: &mut [usize],
    frontier: &mut VecDeque<usize>,
) {
    let next = idx(x, y, width);

    if state.cells[next].inked {
        return;
    }

    // Already visited.
    if dist[next] != usize::MAX {
        return;
    }

    dist[next] = next_dist;
    parent[next] = current;
    frontier.push_back(next);
}

pub fn bfs_best_shortest_path_last(
    state: &State,
    from: (usize, usize),
    to: (usize, usize),
) -> Vec<(usize, usize)> {
    let mut visited = HashSet::new();
    let mut parent: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
    let mut frontier = VecDeque::new();

    visited.insert(from);
    frontier.push_back(from);

    while let Some(node) = frontier.pop_front() {
        if node == to {
            break;
        }

        let (x, y) = node;

        for neighbour in neighbors(x, y, state.width, state.height) {
            if state.cells[idx(neighbour.0, neighbour.1, state.width)].inked {
                continue;
            }

            if visited.insert(neighbour) {
                parent.insert(neighbour, node);
                frontier.push_back(neighbour);
            }
        }
    }

    if !visited.contains(&to) {
        return Vec::new();
    }

    let mut path = vec![to];
    let mut current = to;

    while current != from {
        current = parent[&current];
        path.push(current);
    }

    path.reverse();
    path
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

pub fn town_at(state: &State, x: usize, y: usize) -> Option<usize> {
    state
        .towns
        .iter()
        .find(|t| t.x == x && t.y == y)
        .map(|t| t.id)
}

#[inline]
pub fn is_rail_cell(state: &State, x: usize, y: usize) -> bool {
    let cell = &state.cells[idx(x, y, state.width)];

    cell.town_id.is_some() || cell.track_owner != FREE
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
    let width = state.width;
    let height = state.height;
    let n = width * height;

    let start = idx(from.0, from.1, width);
    let goal = idx(to.0, to.1, width);

    // Cheapest known cost to reach each cell.
    let mut dist = vec![usize::MAX; n];

    // Previous cell in the cheapest path.
    let mut parent = vec![None; n];

    // Min-heap implemented using Reverse.
    //
    // Tuple:
    //     cost
    //     order
    //     cell index
    //
    // `order` gives deterministic tie-breaking.
    let mut heap = BinaryHeap::new();
    let mut order = 0usize;

    dist[start] = 0;

    heap.push(Reverse((0, order, start)));
    order += 1;

    while let Some(Reverse((cost, _, current))) = heap.pop() {
        // This is an outdated entry.
        //
        // A cell can be inserted into the heap multiple times when
        // we discover progressively cheaper paths to it.
        if cost != dist[current] {
            continue;
        }

        // Since this is the cheapest entry currently in the heap,
        // we can stop as soon as we reach the destination.
        if current == goal {
            break;
        }

        let x = current % width;
        let y = current / width;

        // ------------------------------------------------------------
        // NORTH
        // ------------------------------------------------------------
        if y > 0 {
            relax(
                state,
                x,
                y - 1,
                current,
                cost,
                &mut dist,
                &mut parent,
                &mut heap,
                &mut order,
            );
        }

        // ------------------------------------------------------------
        // EAST
        // ------------------------------------------------------------
        if x + 1 < width {
            relax(
                state,
                x + 1,
                y,
                current,
                cost,
                &mut dist,
                &mut parent,
                &mut heap,
                &mut order,
            );
        }

        // ------------------------------------------------------------
        // SOUTH
        // ------------------------------------------------------------
        if y + 1 < height {
            relax(
                state,
                x,
                y + 1,
                current,
                cost,
                &mut dist,
                &mut parent,
                &mut heap,
                &mut order,
            );
        }

        // ------------------------------------------------------------
        // WEST
        // ------------------------------------------------------------
        if x > 0 {
            relax(
                state,
                x - 1,
                y,
                current,
                cost,
                &mut dist,
                &mut parent,
                &mut heap,
                &mut order,
            );
        }
    }

    // No path.
    if dist[goal] == usize::MAX {
        return None;
    }

    // ------------------------------------------------------------
    // Reconstruct path
    // ------------------------------------------------------------

    let mut path = Vec::new();
    let mut current = goal;

    while current != start {
        path.push((current % width, current / width));

        current = parent[current]?;
    }

    // Add starting cell.
    path.push(from);

    // We reconstructed backwards.
    path.reverse();

    Some(path)
}

pub fn best_possible_path(
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
            town_id: None,
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
    let mut connections = HashMap::new();
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
        let id = p[0].parse().unwrap();
        for k in &desired {
            connections.insert((id, *k), Connection::new((id, *k), false));
        }
        towns.push(Town { id, x, y, desired });

        cells[idx(x, y, width)].town_id = Some(p[0].parse().unwrap());
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
        connections,
        my_score: 0,
        foe_score: 0,
        top_cells: [0; 3],
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

    for (k, v) in &mut state.connections {
        *v = Connection::new(v.id, v.dead);
    }
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
                        let x = i % state.width;
                        let y = i / state.width;
                        if let Some(c) = state.connections.get_mut(&(a, b)) {
                            c.active_path.push((x, y));
                        } else {
                            panic!("this should be a registered connection");
                        }
                    }
                }
            }
        }
        let region = state.regions.get_mut(&state.cells[i].region_id);
        if let Some(r) = region {
            if track_owner != -1 && track_owner != 2 && !state.cells[i].inked {
                r.score[track_owner as usize] += (1
                    + 2 * (state.cells[i].active.len() as i32)
                    + 10 * state.cells[i].instability as i32);
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
pub struct Conn {
    pub id: (usize, usize),
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
fn direction_between(from: (usize, usize), to: (usize, usize)) -> Direction {
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
fn best_forward_path(origin: (usize, usize), desired: (usize, usize)) -> Vec<(usize, usize)> {
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
pub fn find_best_initial_connections(state: &State) -> Vec<Conn> {
    // -------------------------------------------------------------
    // Build an ID -> coordinate map.
    //
    // We only need coordinates while constructing connections, so
    // storing references isn't necessary.
    // -------------------------------------------------------------
    let towns: HashMap<usize, (usize, usize)> = state
        .towns
        .iter()
        .map(|town| (town.id, (town.x, town.y)))
        .collect();

    // Count connections first so we can allocate the result once.
    let connection_count: usize = state.towns.iter().map(|town| town.desired.len()).sum();

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

            let forward_direction = direction_between(origin, forward_first_step);

            let reverse_direction = direction_between(desired, reverse_first_step);

            // -----------------------------------------------------
            // STEP 2:
            //
            // Choose orientation by FIRST STEP:
            //
            // NORTH > EAST > SOUTH > WEST
            // -----------------------------------------------------
            let path = if forward_direction.rank() <= reverse_direction.rank() {
                forward_path
            } else {
                forward_path.into_iter().rev().collect()
            };

            connections.push(Conn {
                id: (origin_town.id, desired_id),
                path,
            });
        }
    }

    connections
}

#[inline]
fn relax(
    state: &State,
    x: usize,
    y: usize,
    current: usize,
    current_cost: usize,
    dist: &mut [usize],
    parent: &mut [Option<usize>],
    heap: &mut BinaryHeap<Reverse<(usize, usize, usize)>>,
    order: &mut usize,
) {
    let next = idx(x, y, state.width);
    let cell = &state.cells[next];

    // Inked cells cannot be used.
    if cell.inked {
        return;
    }

    // Existing tracks and towns cost 0.
    //
    // Otherwise we pay the terrain cost.
    let extra_cost = if cell.town_id.is_some() || cell.track_owner != FREE {
        0
    } else {
        terrain_cost(cell.terrain) as usize
    };

    let new_cost = current_cost + extra_cost;

    if new_cost < dist[next] {
        dist[next] = new_cost;
        parent[next] = Some(current);

        heap.push(Reverse((new_cost, *order, next)));

        *order += 1;
    }
}
