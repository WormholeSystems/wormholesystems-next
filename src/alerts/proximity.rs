//! How far a system is from a chain, in gate jumps.
//!
//! A multi-source breadth-first search from every system on the map at once, over the
//! static stargate graph merged with the map's own wormhole connections. Unweighted,
//! because an alert only ever asks "how many jumps", never "which way is safer" — that is
//! the client's weighted router, and porting it would be answering a question nobody asked.
//!
//! Searching from all of the map's systems at once rather than once per system is what
//! keeps this cheap: one traversal answers "how close is the nearest part of my chain".

use std::collections::{HashMap, HashSet, VecDeque};

use sqlx::PgPool;

/// Jovian stargates need access nobody has, so a route through Zarzakh is not a route.
const ZARZAKH: i64 = 30100000;

/// The stargate graph, loaded once and shared by every evaluation.
///
/// Around 8,000 systems and 14,000 gates: small enough to hold, expensive enough that
/// re-reading it per killmail would be the most expensive thing the ingest does.
pub struct Universe {
    adjacency: HashMap<i64, Vec<i64>>,
}

impl Universe {
    pub async fn load(pool: &PgPool) -> sqlx::Result<Universe> {
        let rows = sqlx::query!(
            "select solar_system_id, destination_system_id from stargates
             where solar_system_id <> $1 and destination_system_id <> $1",
            ZARZAKH,
        )
        .fetch_all(pool)
        .await?;
        let mut adjacency: HashMap<i64, Vec<i64>> = HashMap::new();
        for row in rows {
            adjacency
                .entry(row.solar_system_id)
                .or_default()
                .push(row.destination_system_id);
        }
        Ok(Universe { adjacency })
    }

    fn neighbours(&self, id: i64) -> &[i64] {
        self.adjacency.get(&id).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// What the search found: how far, from where, and the way there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proximity {
    pub jumps: i32,
    /// The system on the map the route starts from.
    pub from: i64,
    /// Every system on the way, starting at `from` and ending at the target.
    pub route: Vec<i64>,
}

/// The nearest system on the map to `target`, within `max_jumps`.
///
/// `chain` is the map's own connections, which are edges like any other: a target one gate
/// from a k-space exit at the far end of the chain is one jump away, however deep the chain
/// runs. Wormholes are free rather than a hop, matching what the client's router shows.
pub fn nearest(
    universe: &Universe,
    origins: &[i64],
    chain: &[(i64, i64)],
    target: i64,
    max_jumps: i32,
) -> Option<Proximity> {
    if origins.is_empty() || max_jumps < 0 {
        return None;
    }
    if origins.contains(&target) {
        return Some(Proximity {
            jumps: 0,
            from: target,
            route: vec![target],
        });
    }

    let mut wormholes: HashMap<i64, Vec<i64>> = HashMap::new();
    for (a, b) in chain {
        wormholes.entry(*a).or_default().push(*b);
        wormholes.entry(*b).or_default().push(*a);
    }

    // Reached from where, and via which system, so the route can be walked back.
    let mut came_from: HashMap<i64, i64> = HashMap::new();
    let mut origin_of: HashMap<i64, i64> = HashMap::new();
    let mut distance: HashMap<i64, i32> = HashMap::new();
    let mut queue: VecDeque<i64> = VecDeque::new();
    let mut seeded: HashSet<i64> = HashSet::new();

    // Every mapped system is a starting point at distance zero, and so is anything the
    // chain reaches from one, since crossing a wormhole is not a gate jump.
    let placed: HashSet<i64> = origins.iter().copied().collect();
    let mut stack: Vec<i64> = origins.to_vec();
    while let Some(id) = stack.pop() {
        if !seeded.insert(id) {
            continue;
        }
        distance.insert(id, 0);
        // A system on the map is its own origin. One the chain merely reaches keeps
        // whichever mapped system reached it, so the message names somewhere you are.
        if placed.contains(&id) {
            origin_of.insert(id, id);
        } else {
            origin_of.entry(id).or_insert(id);
        }
        queue.push_back(id);
        for next in wormholes.get(&id).map(Vec::as_slice).unwrap_or(&[]) {
            if !seeded.contains(next) {
                came_from.insert(*next, id);
                origin_of.insert(*next, *origin_of.get(&id).unwrap_or(&id));
                stack.push(*next);
            }
        }
    }
    if distance.contains_key(&target) {
        return Some(Proximity {
            jumps: 0,
            from: *origin_of.get(&target).unwrap_or(&target),
            route: walk_back(&came_from, target),
        });
    }

    while let Some(current) = queue.pop_front() {
        let steps = distance[&current];
        if steps >= max_jumps {
            continue;
        }
        for next in universe.neighbours(current) {
            if distance.contains_key(next) {
                continue;
            }
            distance.insert(*next, steps + 1);
            came_from.insert(*next, current);
            let origin = *origin_of.get(&current).unwrap_or(&current);
            origin_of.insert(*next, origin);
            if *next == target {
                return Some(Proximity {
                    jumps: steps + 1,
                    from: origin,
                    route: walk_back(&came_from, target),
                });
            }
            queue.push_back(*next);
        }
    }
    None
}

fn walk_back(came_from: &HashMap<i64, i64>, target: i64) -> Vec<i64> {
    let mut route = vec![target];
    let mut at = target;
    while let Some(previous) = came_from.get(&at) {
        route.push(*previous);
        at = *previous;
    }
    route.reverse();
    route
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A line of five systems: 1 - 2 - 3 - 4 - 5.
    fn line() -> Universe {
        let mut adjacency: HashMap<i64, Vec<i64>> = HashMap::new();
        for id in 1..5 {
            adjacency.entry(id).or_default().push(id + 1);
            adjacency.entry(id + 1).or_default().push(id);
        }
        Universe { adjacency }
    }

    #[test]
    fn counts_gate_jumps_from_the_nearest_mapped_system() {
        let found = nearest(&line(), &[1, 4], &[], 5, 10).unwrap();
        assert_eq!(found.jumps, 1);
        assert_eq!(found.from, 4);
        assert_eq!(found.route, vec![4, 5]);
    }

    #[test]
    fn a_mapped_system_is_zero_jumps_from_itself() {
        let found = nearest(&line(), &[3], &[], 3, 5).unwrap();
        assert_eq!(found.jumps, 0);
        assert_eq!(found.route, vec![3]);
    }

    #[test]
    fn nothing_beyond_the_limit() {
        assert!(nearest(&line(), &[1], &[], 5, 3).is_none());
        assert_eq!(nearest(&line(), &[1], &[], 5, 4).unwrap().jumps, 4);
    }

    /// The chain is a shortcut: a wormhole from 1 to 4 puts 5 one jump from the map even
    /// though the gates alone would be four.
    #[test]
    fn the_chain_carries_the_search_with_it() {
        let found = nearest(&line(), &[1], &[(1, 4)], 5, 2).unwrap();
        assert_eq!(found.jumps, 1);
        assert_eq!(found.from, 1);
        assert_eq!(found.route, vec![1, 4, 5]);
    }

    #[test]
    fn an_empty_map_is_never_near_anything() {
        assert!(nearest(&line(), &[], &[], 5, 10).is_none());
    }
}
