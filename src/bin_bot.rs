mod common;
use common::*;
use std::io::{self, BufRead, Write};

fn choose_action(state: &State) -> String {
    // League 1 strategy:
    // Find any desired town pair that is not yet connected and use AUTOPLACE.
    // This is intentionally simple so you can replace it with your real AI.
    for a in &state.towns {
        for &b_id in &a.desired {
            if let Some(b) = state.towns.iter().find(|t| t.id == b_id) {
                if shortest_active_path(state, (a.x,a.y), (b.x,b.y)).is_none() {
                    return format!("AUTOPLACE {} {} {} {}", a.x,a.y,b.x,b.y);
                }
            }
        }
    }
    "WAIT".to_string()
}

fn main() {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut state = match parse_initial(&mut input) {
        Ok(s) => s,
        Err(e) => { eprintln!("initialization error: {e}"); return; }
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();

    loop {
        match read_turn(&mut input, &mut state) {
            Ok(()) => {}
            Err(e) => { eprintln!("turn read error: {e}"); break; }
        }
        let action = choose_action(&state);
        writeln!(out, "{action}").unwrap();
        out.flush().unwrap();
    }
}
