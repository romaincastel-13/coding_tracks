mod common;
use std::collections::{HashMap, VecDeque, HashSet};

use crate::common::*;

fn main() {

    let mut towns = Vec::new();
    towns.push(
        Town {
            id: towns.len() as i32,
            x : 6,
            y: 0,
            desired: vec![1]
        }
    );
    towns.push(
        Town {
            id: towns.len() as i32,
            x : 3,
            y: 3,
            desired: Vec::new()
        }
    );

    let state = State {
        my_id: 0,
        foe_id: 1,
        width: 30,
        height: 30,
        cells: Vec::new(),
        towns,
        connections: HashSet::new(),
        regions: HashMap::new(),
        my_score: 0,
        foe_score: 0,
    };

    let res = find_best_initial_connections(&state);
    println!("{res:#?}");

}