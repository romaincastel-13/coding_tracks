use crate::common;
use std::collections::HashSet;

/// Simulates a single turn given an action string (as the bot would output).
/// Returns the new state at the start of the next turn.
///
/// The action string is a semicolon-separated list of commands.
/// Only one AUTOPLACE is allowed per turn.
/// PLACE_TRACKS actions are processed before DISRUPT actions.
/// Paint points: 3 per player per turn.
/// Disruption points: 1 per player per turn.
pub fn apply_turn(state: &State, action_str: &str) -> State {
    // Clone the state to avoid mutating the original
    let mut new_state = state.clone();

    // Parse the action string into a list of actions
    let actions = parse_actions(action_str);
    if actions.is_empty() {
        // No actions, just wait
        // But we still need to update scores? Actually if no actions, we still need to
        // compute points from current state? No, points are scored after placements and disruptions.
        // If there are no placements and no disruptions, the state remains the same except
        // that we still need to compute points? Actually points are scored at the end of each turn
        // based on active connections after inking. If we do nothing, the board does not change,
        // so the active connections and scores remain the same as previous turn? Wait:
        // The scores in the state are cumulative up to the previous turn. If we do nothing,
        // we still need to compute the points earned this turn (which would be zero) and add to scores.
        // However, the server would still send updated scores (same as before) because no new points.
        // So we need to compute points from the current state (after applying any inking? but inking
        // only changes via disruption, which we didn't do). So we can just compute points from
        // the current state (as it is at start of turn) and add zero? Actually the points earned
        // this turn are based on the board after our actions. If we do nothing, the board is unchanged,
        // so the points earned this turn are the same as the points that were already earned
        // in previous turns? No, points are earned each turn based on active connections that turn.
        // If we do nothing, the active connections may still change because opponent might have
        // placed tracks? But we are only simulating our own actions; we assume opponent does nothing.
        // For simplicity, in this simulation we assume we are the only player acting (opponent passes).
        // This is appropriate for move generation where we consider our own actions only.
        // We'll simulate opponent as doing nothing (no placements, no disruptions).
        // We'll implement that by calling apply_turn_with_opponent with empty actions for opponent.
        // For now, we'll implement a helper that simulates both players.
        // Let's refactor: we'll create a function that simulates a turn given actions for both players.
        // Then apply_turn for our player alone will call that with opponent actions empty.
        // We'll do that below.
        return apply_turn_both(state, &[], &[]);
    }

    // Separate actions into placements (including from autoplace) and disruptions
    let (placements, disruptions) = separate_actions(&actions);
    // For now, assume opponent does nothing
    let opponent_placements: Vec<Action> = Vec::new();
    let opponent_disruptions: Vec<Action> = Vec::new();

    apply_turn_both(state, &placements, &disruptions, &opponent_placements, &opponent_disruptions)
}

/// Separates actions into placements (including expanded autoplace) and disruptions.
/// WAIT and MESSAGE are ignored.
fn separate_actions(actions: &[Action]) -> (Vec<Action>, Vec<Action>) {
    let mut placements = Vec::new();
    let mut disruptions = Vec::new();
    let mut used_autoplace = false;

    for action in actions {
        match action {
            Action::Place(_, _) => placements.push(*action),
            Action::Disrupt(_) => disruptions.push(*action),
            Action::Auto(from_x, from_y, to_x, to_y) => {
                if used_autoplace {
                    // Only one autoplace allowed per turn; ignore extra
                    continue;
                }
                used_autoplace = true;
                // Expand autoplace into placements (using logic from bin_runner.rs)
                let expanded = expand_autoplace(state, Action::Auto(*from_x, *from_y, *to_x, *to_y));
                placements.extend(expanded);
            }
            Action::Wait => {}
            // MESSAGE is not in Action enum (it's omitted), so we don't need to handle it.
        }
    }

    (placements, disruptions)
}

/// Simulates a turn given actions for both players.
/// Each argument is a list of primitive actions (Place, Disrupt) for that player.
/// The function assumes that autoplace has already been expanded into placements.
fn apply_turn_both(
    state: &State,
    my_placements: &[Action],
    my_disruptions: &[Action],
    opp_placements: &[Action],
    opp_disruptions: &[Action],
) -> State {
    let mut new_state = state.clone();

    // Step 1: Apply placements simultaneously for both players
    let (score_after_place_my, score_after_place_opp) =
        apply_placements(&mut new_state, my_placements, opp_placements);

    // Step 2: Apply disruptions simultaneously for both players
    apply_disruptions(&mut new_state, my_disruptions, opp_disruptions);

    // Step 3: Ink out regions where instability >= 4
    ink_regions(&mut new_state);

    // Step 4: Recompute active connections based on remaining tracks
    recompute_active(&mut new_state);

    // Step 5: Compute points earned from active connections this turn
    let (points_my, points_opp) = score_connections(&new_state);

    // Step 6: Update scores (cumulative)
    new_state.my_score += points_my;
    new_state.foe_score += points_opp;

    // Step 7: Update region scores (as per read_turn formula)
    update_region_scores(&mut new_state);

    new_state
}

/// Apply placements simultaneously, respecting paint point limits and conflict resolution.
/// Returns the points earned from connections after placements but before disruptions and inking?
/// Actually we don't need points here; we'll compute later after all steps.
/// We just update the track_owner on the state.
fn apply_placements(
    state: &mut State,
    my_placements: &[Action],
    opp_placements: &[Action],
) -> (i32, i32) {
    // We'll follow the logic from bin_runner.rs::apply_simultaneous but adapted for two players.
    let mut used = [0i32; 2]; // paint points used by player 0 and 1
    let mut requests: std::collections::HashMap<(usize, usize), Vec<usize>> =
        std::collections::HashMap::new();

    // Helper to try to place a track for a player
    fn try_place(
        state: &State,
        player_id: usize,
        x: usize,
        y: usize,
        used: &mut [i32; 2],
        requests: &mut std::collections::HashMap<(usize, usize), Vec<usize>>,
    ) -> bool {
        if x >= state.width || y >= state.height {
            return false;
        }
        // Cannot place on a town
        if town_at(state, x, y).is_some() {
            return false;
        }
        let idx = common::idx(x, y, state.width);
        let cell = &state.cells[idx];
        // Cannot place on existing track or inked region
        if cell.track_owner != common::FREE || cell.inked {
            return false;
        }
        let cost = terrain_cost(cell.terrain);
        if used[player_id] + cost > 3 {
            return false;
        }
        used[player_id] += cost;
        requests.entry((x, y)).or_default().push(player_id);
        true
    }

    // Process my placements (player 0)
    for action in my_placements {
        if let Action::Place(x, y) = action {
            try_place(state, 0, *x, *y, &mut used, &mut requests);
        }
    }
    // Process opponent placements (player 1)
    for action in opp_placements {
        if let Action::Place(x, y) = action {
            try_place(state, 1, *x, *y, &mut used, &mut requests);
        }
    }

    // Resolve conflicts and apply placements
    for ((x, y), owners) in requests {
        let owner = if owners.len() >= 2 {
            common::NEUTRAL
        } else {
            owners[0] as i32
        };
        let idx = common::idx(x, y, state.width);
        state.cells[idx].track_owner = owner;
    }

    // We don't return points here because points are computed after all steps.
    (0, 0)
}

/// Apply disruptions: each disruption increases instability of the region by 1.
fn apply_disruptions(
    state: &mut State,
    my_disruptions: &[Action],
    opp_disruptions: &[Action],
) {
    for action in my_disruptions {
        if let Action::Disrupt(region_id) = action {
            // Disrupt by region_id
            if let Some(region) = state.regions.get_mut(region_id) {
                // Check if region can be disrupted: no town and not already inked
                if !region.has_town {
                    // We need to increase instability of all cells in the region by 1
                    // But instability is stored per cell, and all cells in a region share the same instability value?
                    // Actually in the state, each cell has its own instability field, but they are supposed to be equal within a region.
                    // We'll increase instability for each cell in the region.
                    for &cell_id in &region.cell_ids {
                        let cell = &mut state.cells[cell_id];
                        if !cell.inked {
                            cell.instability += 1;
                        }
                    }
                }
            }
        }
    }
    for action in opp_disruptions {
        if let Action::Disrupt(region_id) = action {
            if let Some(region) = state.regions.get_mut(region_id) {
                if !region.has_town {
                    for &cell_id in &region.cell_ids {
                        let cell = &mut state.cells[cell_id];
                        if !cell.inked {
                            cell.instability += 1;
                        }
                    }
                }
            }
        }
    }
}

/// Ink out regions where instability >= 4: wash away tracks and set inked = true.
fn ink_regions(state: &mut State) {
    for region in state.regions.values() {
        // Check if region should be inked: instability >= 4 and not already inked?
        // We'll check the instability of the first cell (they should be same)
        if let Some(first_cell_id) = region.cell_ids.first() {
            let cell = &state.cells[*first_cell_id];
            if cell.instability >= 4 && !cell.inked {
                // Ink the region: set inked = true and wash away tracks
                for &cell_id in &region.cell_ids {
                    let cell = &mut state.cells[cell_id];
                    cell.inked = true;
                    cell.track_owner = common::FREE; // wash away tracks
                    // Note: active connections will be cleared later by recompute_active
                }
            }
        }
    }
}

/// Recompute active connections (mutates state)
fn recompute_active(state: &mut State) {
    // Clear active vectors
    for cell in &mut state.cells {
        cell.active.clear();
    }

    // Each unilateral desired connection is an independent connection.
    for a in &state.towns {
        for &b_id in &a.desired {
            if let Some(b) = state.towns.iter().find(|t| t.id == b_id) {
                if let Some(path) = shortest_active_path(state, (a.x, a.y), (b.x, b.y)) {
                    for &(x, y) in &path {
                        let idx = common::idx(x, y, state.width);
                        state.cells[idx].active.push((a.id, b.id));
                    }
                }
            }
        }
    }
}

/// Update region scores as per read_turn formula.
fn update_region_scores(state: &mut State) {
    // First reset region scores to zero
    for region in state.regions.values_mut() {
        region.score = [0, 0];
    }

    // Then accumulate
    for cell in &state.cells {
        let idx = cell.cell_id;
        if cell.track_owner != common::FREE && cell.track_owner != common::NEUTRAL && !cell.inked {
            let region = state.regions.get_mut(&cell.region_id).unwrap();
            region.score[cell.track_owner as usize] +=
                (1 + 2 * (cell.active.len() as i32) + 10 * cell.instability);
        }
    }
}

/// Expand an AUTOPLACE action into a list of PLACE_TRACKS actions, respecting
/// that if a path already exists, it does nothing.
/// This is adapted from bin_runner.rs::expand_autoplace.
fn expand_autoplace(state: &State, action: Action) -> Vec<Action> {
    match action {
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
                    town_at(state, x, y).is_none() &&
                        state.cells[common::idx(x, y, state.width)].track_owner == common::FREE
                })
                .map(|(x, y)| Action::Place(x, y))
                .collect()
        }
        _ => vec![action],
    }
}

/// Parses a string into a list of actions (same as common::Action::parse but returns vec)
fn parse_actions(s: &str) -> Vec<Action> {
    s.split(';')
        .filter_map(|s| Action::parse(s.trim()).ok())
        .collect()
}

/// Returns all legal primitive actions (single commands) that can be issued legally
/// assuming no other actions are issued in the same turn (i.e., the action string consists
/// of just this command). This is useful for move generation where we consider one
/// action at a time.
pub fn legal_actions(state: &State) -> Vec<Action> {
    let mut actions = Vec::new();

    // WAIT is always legal
    actions.push(Action::Wait);

    // MESSAGE is omitted; we ignore.

    // PLACE_TRACKS: legal if cell is free, not a town, not inked, and cost <= remaining paint points (3)
    // Since we consider the action alone, remaining paint points is 3.
    for y in 0..state.height {
        for x in 0..state.width {
            if let Action::Place(x, y) = Action::Place(x, y) {
                // Check if we can place
                if x >= state.width || y >= state.height {
                    continue;
                }
                if town_at(state, x, y).is_some() {
                    continue;
                }
                let idx = common::idx(x, y, state.width);
                let cell = &state.cells[idx];
                if cell.track_owner != common::FREE || cell.inked {
                    continue;
                }
                let cost = terrain_cost(cell.terrain);
                if cost <= 3 {
                    actions.push(Action::Place(x, y));
                }
            }
        }
    }

    // AUTOPLACE: legal if there is no existing path and the cheapest path exists
    // and the generated placements would be legal (we'll just check that the cheapest path
    // does not go through towns and that each step is placeable with sufficient paint points?).
    // For simplicity, we'll generate the autoplace and see if it yields any placements.
    // We'll also need to ensure that the total paint cost of the generated placements <= 3.
    // We'll iterate over all pairs of towns? Actually AUTOPLACE takes any two coordinates.
    // That's too many. We'll limit to reasonable pairs: maybe only from town to desired town?
    // But the spec allows any coordinates. For move generation, we might want to restrict
    // to meaningful actions. However, for completeness we can generate all possible
    // coordinate pairs, but that's O((width*height)^2) which is up to (600)^2 = 360k, manageable.
    // We'll do it.
    for y1 in 0..state.height {
        for x1 in 0..state.width {
            for y2 in 0..state.height {
                for x2 in 0..state.width {
                    let action = Action::Auto(x1, y1, x2, y2);
                    let expanded = expand_autoplace(state, action);
                    if !expanded.is_empty() {
                        // Check total paint cost
                        let mut cost = 0;
                        for a in &expanded {
                            if let Action::Place(x, y) = a {
                                let idx = common::idx(*x, *y, state.width);
                                cost += terrain_cost(state.cells[idx].terrain);
                            }
                        }
                        if cost <= 3 {
                            actions.push(action);
                        }
                    }
                }
            }
        }
    }

    // DISRUPT: legal if region has no town and is not already inked
    for (region_id, region) in &state.regions {
        if !region.has_town {
            // Check if region is not already inked (check first cell's inked)
            if let Some(first_id) = region.cell_ids.first() {
                let cell = &state.cells[*first_id];
                if !cell.inked {
                    actions.push(Action::Disrupt(*region_id));
                }
            }
        }
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legal_actions() {
        // We'll need to create a test state; for now just ensure it compiles.
        let state = State {
            my_id: 0,
            foe_id: 1,
            width: 1,
            height: 1,
            cells: vec![Cell {
                region_id: 0,
                cell_id: 0,
                terrain: common::PLAINS,
                track_owner: common::FREE,
                instability: 0,
                inked: false,
                active: Vec::new(),
            }],
            towns: Vec::new(),
            regions: std::collections::HashMap::from([(0, common::Region {
                cell_ids: vec![0],
                score: [0, 0],
                has_town: false,
            })]),
            connections: HashSet::new(),
            my_score: 0,
            foe_score: 0,
        };
        let actions = legal_actions(&state);
        // Should at least contain WAIT and a place action
        assert!(actions.contains(&Action::Wait));
        assert!(actions.contains(&Action::Place(0, 0)));
    }
}