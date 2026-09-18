mod common;

use crate::common::*;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const WIDTH: usize = 21;
const HEIGHT: usize = 14;
const MAX_TURNS: usize = 100;

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

fn make_map(seed: u64) -> State {
    let mut rng = Rng::new(seed);
    let mut cells = Vec::with_capacity(WIDTH * HEIGHT);
    let mut i = 0;

    // Simple deterministic regions: 3x2-ish blocks, with random terrain.
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let region_id = ((x / 3) + (y / 3) * ((WIDTH + 2) / 3)) as i32;
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
        let x = rng.range(WIDTH);
        let y = rng.range(HEIGHT);
        if cells[idx(x, y, WIDTH)].terrain != PLAINS {
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
        let region = cells[idx(x, y, WIDTH)].region_id;
        if positions
            .iter()
            .any(|&(px, py)| cells[idx(px, py, WIDTH)].region_id == region)
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
        width: WIDTH,
        height: HEIGHT,
        cells,
        towns,
        connections: HashMap::new(),
        regions: HashMap::new(),
        my_score: 0,
        foe_score: 0,
        top_cells: [0; 3]
    }
}

fn send_initial<W: Write>(w: &mut W, state: &State, my_id: i32) {
    writeln!(w, "{my_id}").unwrap();
    writeln!(w, "{}", state.width).unwrap();
    writeln!(w, "{}", state.height).unwrap();
    for c in &state.cells {
        writeln!(w, "{} {}", c.region_id, c.terrain).unwrap();
    }
    writeln!(w, "{}", state.towns.len()).unwrap();
    for t in &state.towns {
        let desired = if t.desired.is_empty() {
            "x".to_string()
        } else {
            t.desired
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        writeln!(w, "{} {} {} {}", t.id, t.x, t.y, desired).unwrap();
    }
    w.flush().unwrap();
}

fn send_turn<W: Write>(w: &mut W, state: &State, my_score: i32, foe_score: i32) {
    writeln!(w, "{my_score}").unwrap();
    writeln!(w, "{foe_score}").unwrap();
    for c in &state.cells {
        let active = if c.active.is_empty() {
            "x".to_string()
        } else {
            c.active
                .iter()
                .map(|(a, b)| format!("{a}-{b}"))
                .collect::<Vec<_>>()
                .join(",")
        };
        writeln!(
            w,
            "{} {} {} {}",
            c.track_owner, c.instability, c.inked as i32, active
        )
        .unwrap();
    }
    w.flush().unwrap();
}

fn expand_autoplace(state: &State, a: &Action) -> Vec<Action> {
    match *a {
        Action::Auto(x1, y1, x2, y2) => {
            if x1 >= state.width || x2 >= state.width || y1 >= state.height || y2 >= state.height {
                return vec![];
            }
            if shortest_active_path(state, (x1, y1), (x2, y2)).is_some() {
                return vec![];
            }
            let path = match cheapest_path(state, (x1, y1), (x2, y2)) {
                Some(p) => p,
                None => return vec![],
            };
            path.into_iter()
                .filter(|&(x, y)| {
                    town_at(state, x, y).is_none()
                        && state.cells[idx(x, y, state.width)].track_owner == FREE
                })
                .map(|(x, y)| Action::Place(x, y))
                .collect()
        }
        _ => vec![a.clone()],
    }
}

fn parse_actions(line: &str) -> Vec<Action> {
    line.split(';')
        .filter_map(|s| Action::parse(s.trim()).ok())
        .collect()
}

fn action_place_cost(state: &State, x: usize, y: usize) -> Option<i32> {
    if x >= state.width || y >= state.height {
        return None;
    }
    if town_at(state, x, y).is_some() {
        return None;
    }
    let c = &state.cells[idx(x, y, state.width)];
    if c.track_owner != FREE || c.inked {
        return None;
    }
    Some(terrain_cost(c.terrain))
}

fn apply_simultaneous(state: &mut State, a0: &[Action], a1: &[Action]) -> (i32, i32) {
    let mut used = [0i32; 2];
    let mut requests: HashMap<(usize, usize), Vec<usize>> = HashMap::new();

    for (pid, actions) in [(0usize, a0), (1usize, a1)] {
        for action in actions {
            if let Action::Place(x, y) = *action {
                if let Some(cost) = action_place_cost(state, x, y) {
                    if used[pid] + cost <= 3 {
                        used[pid] += cost;
                        requests.entry((x, y)).or_default().push(pid);
                    }
                }
            }
        }
    }

    for ((x, y), owners) in requests {
        let owner = if owners.len() >= 2 {
            NEUTRAL
        } else {
            owners[0] as i32
        };
        state.cells[idx(x, y, state.width)].track_owner = owner;
    }

    recompute_active(state);
    score_connections(state)
}

fn print_map(state: &State) {
    println!();
    println!("Map:");
    for y in 0..state.height {
        for x in 0..state.width {
            let ch = if let Some(id) = town_at(state, x, y) {
                char::from(b'0' + (id as u8 % 10))
            } else if state.cells[idx(x, y, state.width)].inked {
                'X'
            } else {
                match state.cells[idx(x, y, state.width)].track_owner {
                    -1 => match state.cells[idx(x, y, state.width)].terrain {
                        PLAINS => '.',
                        RIVER => '~',
                        MOUNTAIN => '^',
                        _ => '?',
                    },
                    0 => 'A',
                    1 => 'B',
                    2 => '#',
                    _ => '?',
                }
            };
            print!("{ch}");
        }
        println!();
    }
    println!();
}

fn run_one(seed: u64, bot_path: &str) -> bool {
    let mut state = make_map(seed);
    eprintln!("Starting bot: {:?}", bot_path);
    let mut child = Command::new(bot_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to start bot");

    let mut sin: std::process::ChildStdin = child.stdin.take().unwrap();
    let sout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(sout);

    send_initial(&mut sin, &state, 0);

    let mut score0 = 0;
    let mut score1 = 0;

    for turn in 1..=MAX_TURNS {
        // Boss AI skips its turn.
        send_turn(&mut sin, &state, score0, score1);

        let start = Instant::now();
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            eprintln!("Bot exited on turn {turn}");
            let _ = child.kill();
            return false;
        }
        let elapsed = start.elapsed();
        if elapsed > Duration::from_millis(50) {
            eprintln!("WARNING: bot took {:?} on turn {turn}", elapsed);
        }

        let actions = parse_actions(line.trim());
        if actions.is_empty() {
            eprintln!("Invalid/no action on turn {turn}: {:?}", line.trim());
            continue;
        }

        // AUTOPLACE expands into normal placements before simultaneous resolution.
        let mut expanded = Vec::new();
        let mut autoplace_count = 0;
        for a in &actions {
            if matches!(a, Action::Auto(..)) {
                autoplace_count += 1;
                if autoplace_count > 1 {
                    continue;
                }
            }
            expanded.extend(expand_autoplace(&state, a));
        }

        let (_new0, _new1) = apply_simultaneous(&mut state, &expanded, &[]);

        // Recalculate scores from scratch as a safety check.
        let (calc0, calc1) = score_connections(&state);
        score0 = calc0;
        score1 = calc1;

        println!("Turn {turn:3}: {line}", line = line.trim());
        println!("Score: {score0} - {score1}");
        print_map(&state);

        if any_active_connection(&state) {
            println!("WIN on turn {turn} (seed {seed})");
            let _ = child.kill();
            return true;
        }
    }

    println!("LOSE: no active connection in {MAX_TURNS} turns (seed {seed})");
    let _ = child.kill();
    false
}

fn main() {
    let bot = std::env::args().nth(1).unwrap_or_else(|| {
        if cfg!(windows) {
            "target/debug/bot.exe".into()
        } else {
            "target/debug/bot".into()
        }
    });

    let mut wins = 0;
    for seed in 16..=16 {
        println!("========================================");
        println!("GAME {seed}");
        println!("========================================");
        if run_one(seed, &bot) {
            wins += 1;
        }
    }

    println!("========================================");
    println!("RESULT: {wins}/5 wins");
    if wins >= 3 {
        println!("League objective achieved.");
    } else {
        println!("League objective failed.");
    }
}
