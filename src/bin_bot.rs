mod common;

use crate::common::*;
use std::fmt::Write as FmtWrite;
use std::io::{self, BufRead, Write};

fn choose_action_place_old(state: &State) -> String {
    // League 1 strategy:
    // Find any desired town pair that is not yet connected and use AUTOPLACE.
    // This is intentionally simple so you can replace it with your real AI.
    for a in &state.towns {
        for &b_id in &a.desired {
            if let Some(b) = state.towns.iter().find(|t| t.id == b_id) {
                if shortest_active_path(state, (a.x, a.y), (b.x, b.y)).is_none()
                    && !state.has_connection(a, b)
                {
                    return format!("AUTOPLACE {} {} {} {}", a.x, a.y, b.x, b.y);
                }
            }
        }
    }
    "WAIT".to_string()
}

fn choose_action_place(state: &mut State, player: usize) -> String {
    state.analyse_connections();
    state.score_cells(player);
    
    let mut out = String::new();
    for i in (0..3).rev() {
        let cell_id = state.cells[state.top_cells[i]].cell_id;
        eprintln!("cell {cell_id:?}");
        out.push_str(&format!(
            "PLACE_TRACKS {} {};",
            cell_id % state.width,
            cell_id / state.width
        ));
    }
    out
}

fn choose_action_disrupt(state: &State) -> String {

    let (a, b) = if state.my_id == 0 { (1, 0) } else { (0, 1) };
    let max = state.regions.iter().max_by_key(|(id, region)| {
        // your definition of "max"
        if region.has_town || region.inked(state) {
            0
        } else {
            region.score[a] - region.score[b]
        }
    });
    if let Some((key, value)) = max {
        return format!("DISRUPT {key}");
    }
    "WAIT".to_string()
}

fn choose_action(state: &mut State) -> String {
    let mut a = choose_action_disrupt(state);
    let player = state.my_id as usize;
    let b = choose_action_place(state, player);
    a.push_str(";");
    a.push_str(&b);
    a
}

fn main() {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    
    let mut state = match parse_initial(&mut input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("initialization error: {e}");
            return;
        }
    };

    // let w = &state.foe_id;

    // eprintln!("state: {w:?}");

    let stdout = io::stdout();
    let mut out = stdout.lock();
    
    loop {
        match read_turn(&mut input, &mut state) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("turn read error: {e}");
                break;
            }
        }
        let action = choose_action(&mut state);
        writeln!(out, "{action}").unwrap();
        out.flush().unwrap();
    }
}
