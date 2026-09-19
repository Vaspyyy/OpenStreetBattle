use osb_sim::*;
use osb_world::{Bounds, Point};
use std::{error::Error, time::Instant};
/// Quiet broad-phase/storage probe, intentionally distinct from tactical combat benchmarking.
pub fn run() -> Result<(), Box<dyn Error>> {
    let base = Simulation::new(Scenario::demo(), 42)?.snapshot();
    let mut rows = Vec::new();
    for count in [100usize, 500, 1000, 5000, 10000] {
        let mut state = base.clone();
        state.scenario.forces.truncate(1);
        state.scenario.forces[0].soldiers = count as u32;
        state.scenario.map.bounds = Bounds {
            min: Point::new(0.0, 0.0),
            max: Point::new(9000.0, 9000.0),
        };
        state.scenario.map.obstacles.clear();
        state.scenario.map.roads.clear();
        let mut root = state.formations[0].clone();
        let mut team = state.formations[2].clone();
        team.parent = Some(root.id);
        root.nominal_strength = count as u32;
        team.nominal_strength = count as u32;
        root.leader = Some(SoldierId(1));
        team.leader = Some(SoldierId(1));
        let template = state.soldiers[0].clone();
        let side = (count as f64).sqrt().ceil() as usize;
        state.soldiers = (0..count)
            .map(|i| {
                let mut s = template.clone();
                s.id = SoldierId(i as u64 + 1);
                s.name = format!("Probe {}", s.id.0);
                s.position = Point::new(
                    40.0 + (i % side) as f64 * 80.0,
                    40.0 + (i / side) as f64 * 80.0,
                );
                s.assigned_formation = team.id;
                s.current_group = team.id;
                s.relationships.clear();
                s.contacts.clear();
                s.radio = i == 0;
                s
            })
            .collect();
        state.formations = vec![root, team];
        state.recent_events.clear();
        state.next_event = 1;
        let started = Instant::now();
        let mut sim = Simulation::restore(state)?;
        let restore_seconds = started.elapsed().as_secs_f64();
        let started = Instant::now();
        sim.advance(5)?;
        let elapsed = started.elapsed().as_secs_f64();
        rows.push(serde_json::json!({"people":count,"workload":"dispersed no-contact reference updates, five ticks; NOT full combat or dormant LOD","restore_seconds":restore_seconds,"seconds_per_tick":elapsed/5.0,"persistent_people":sim.soldiers().len()}));
    }
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}
