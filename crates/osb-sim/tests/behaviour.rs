use osb_sim::*;
use osb_world::Point;
fn close_contact() -> Simulation {
    let mut s = Scenario::demo();
    s.map.obstacles.clear();
    s.forces[0].soldiers = 8;
    s.forces[1].soldiers = 8;
    s.forces[0].spawn = Point::new(250.0, 100.0);
    s.forces[1].spawn = Point::new(420.0, 100.0);
    for f in &mut s.forces {
        f.intent.target = Point::new(350.0, 100.0);
    }
    Simulation::new(s, 42).unwrap()
}
#[test]
fn contact_is_shared_without_shooting_through_cover() {
    let mut s = Scenario::demo();
    s.forces[0].soldiers = 2;
    s.forces[1].soldiers = 1;
    let sim = Simulation::new(s, 42).unwrap();
    let mut snap = sim.snapshot();
    snap.soldiers[0].position = Point::new(350.0, 210.0);
    snap.soldiers[1].position = Point::new(350.0, 260.0);
    snap.soldiers[2].position = Point::new(550.0, 210.0);
    let hidden = snap.soldiers[1].id;
    let enemy = snap.soldiers[2].id;
    let mut sim = Simulation::restore(snap).unwrap();
    sim.step().unwrap();
    assert!(
        sim.inspect(hidden)
            .unwrap()
            .contacts
            .iter()
            .any(|c| c.enemy == enemy && c.shared)
    );
    assert!(
        !sim.events()
            .iter()
            .any(|e| e.kind == EventKind::Shot && e.actor == Some(hidden))
    );
}
#[test]
fn ammunition_is_conserved_except_for_recorded_shots() {
    let mut s = close_contact();
    let before: u64 = s.statistics().iter().map(|f| f.rounds).sum();
    s.advance(30).unwrap();
    let after: u64 = s.statistics().iter().map(|f| f.rounds).sum();
    let shots = s
        .events()
        .iter()
        .filter(|e| e.kind == EventKind::Shot)
        .count() as u64;
    assert!(shots > 0);
    assert_eq!(before - after, shots);
}
#[test]
fn checkpoint_during_actual_combat_is_exact() {
    let mut a = close_contact();
    a.advance(120).unwrap();
    assert!(a.events().iter().any(|e| e.kind == EventKind::Injury));
    let text = serde_json::to_string(&a.snapshot()).unwrap();
    let mut b = Simulation::restore(serde_json::from_str(&text).unwrap()).unwrap();
    a.advance(200).unwrap();
    b.advance(200).unwrap();
    assert_eq!(a.fingerprint().unwrap(), b.fingerprint().unwrap());
    a.snapshot().validate().unwrap();
}
#[test]
fn medical_items_are_conserved_except_for_treatment() {
    let mut s = close_contact();
    let before: u32 = s
        .soldiers()
        .iter()
        .map(|s| u32::from(s.inventory.medical_supplies))
        .sum();
    s.advance(300).unwrap();
    let after: u32 = s
        .soldiers()
        .iter()
        .map(|s| u32::from(s.inventory.medical_supplies))
        .sum();
    let used = s
        .events()
        .iter()
        .filter(|e| e.kind == EventKind::Treatment)
        .count() as u32;
    assert_eq!(before - after, used);
}

#[test]
fn decision_diagnostics_do_not_change_authoritative_outcomes() {
    let mut traced = osb_sim::Simulation::new(osb_sim::Scenario::demo(), 42).unwrap();
    let mut silent = osb_sim::Simulation::new(osb_sim::Scenario::demo(), 42).unwrap();
    silent.set_diagnostics_enabled(false);
    traced.advance(3200).unwrap();
    silent.advance(3200).unwrap();
    assert_eq!(traced.fingerprint().unwrap(), silent.fingerprint().unwrap());
    for soldier in traced.soldiers() {
        let history = traced.decisions(soldier.id).unwrap();
        assert!(!history.is_empty() && history.len() <= osb_sim::DECISION_HISTORY);
        assert!(silent.decisions(soldier.id).is_none());
    }
}

#[test]
fn decision_trace_records_actual_objective_branch_and_has_no_invented_enemy() {
    let mut sim = osb_sim::Simulation::new(osb_sim::Scenario::demo(), 1).unwrap();
    sim.step().unwrap();
    let trace = sim
        .decisions(osb_sim::SoldierId(1))
        .unwrap()
        .back()
        .unwrap();
    assert_eq!(
        trace.reason,
        osb_sim::DecisionReason::MoveToAssignedObjective
    );
    assert_eq!(trace.known_contacts, 0);
    assert!(trace.known_threat.is_none());
    assert_eq!(trace.tick, 1);
    assert!(trace.destination.is_some());
    let restored = osb_sim::Simulation::restore(sim.snapshot()).unwrap();
    assert!(restored.decisions(osb_sim::SoldierId(1)).is_none());
}
