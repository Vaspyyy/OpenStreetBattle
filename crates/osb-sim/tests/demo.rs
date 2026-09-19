use osb_sim::{EventKind, Scenario, Simulation, TICKS_PER_SECOND};

#[test]
fn bundled_demo_reaches_gunfire_without_losing_identities() {
    let mut sim = Simulation::new(Scenario::demo(), 42).unwrap();
    let identities: Vec<_> = sim.soldiers().iter().map(|s| s.id).collect();
    let mut fired = false;
    for _ in 0..8 * 60 * TICKS_PER_SECOND {
        sim.step().unwrap();
        if sim.events().iter().any(|e| e.kind == EventKind::Shot) {
            fired = true;
            break;
        }
    }
    assert!(fired, "the bundled demo must reach combat, not merely tick");
    assert_eq!(
        identities,
        sim.soldiers().iter().map(|s| s.id).collect::<Vec<_>>()
    );
}
