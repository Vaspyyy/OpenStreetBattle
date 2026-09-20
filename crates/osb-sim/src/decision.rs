//! Bounded observational telemetry, recorded at the decision branches.
//! Never consulted by AI, serialized into authoritative state, or given RNG access.
use crate::{Action, Directive, Formation, Soldier, SoldierId};
use osb_world::Point;
use std::collections::{BTreeMap, VecDeque};

pub const DECISION_HISTORY: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionReason {
    Dead,
    Unconscious,
    AlreadySurrendered,
    IsolatedAndOverwhelmed,
    LowMorale,
    CommanderWithdrawal,
    TreatSelf,
    MedicAssistance,
    HelpBondedComrade,
    SuppressedByKnownContact,
    EngageKnownContact,
    MoveToAssignedObjective,
    HoldAtAssignedObjective,
}
impl DecisionReason {
    pub fn description(self) -> &'static str {
        match self {
            Self::Dead => "No action: deceased",
            Self::Unconscious => "No action: unconscious or incapacitated",
            Self::AlreadySurrendered => "Surrendered earlier; no further combat orders",
            Self::IsolatedAndOverwhelmed => {
                "Very low morale, no nearby active allies, and a close known threat"
            }
            Self::LowMorale => "Morale below the individual withdrawal threshold",
            Self::CommanderWithdrawal => "Current formation ordered withdrawal",
            Self::TreatSelf => "Bleeding, supplies available, and suppression permits self-aid",
            Self::MedicAssistance => "Medic selected a visible bleeding ally",
            Self::HelpBondedComrade => {
                "Bond and courage thresholds permit helping this bleeding ally"
            }
            Self::SuppressedByKnownContact => {
                "Suppression above threshold while an enemy contact is known"
            }
            Self::EngageKnownContact => {
                "Known contact, ammunition, weapon range and clear shot geometry"
            }
            Self::MoveToAssignedObjective => "Outside the assigned objective's holding distance",
            Self::HoldAtAssignedObjective => "Inside the assigned objective's holding distance",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavigationOutcome {
    #[default]
    NotRequested,
    Moved,
    Arrived,
    InvalidDestination,
    NoRoute,
    WaitingToRepath,
    BlockedSegment,
}
#[derive(Clone, Debug)]
pub struct DecisionRecord {
    pub tick: u64,
    pub entered_at: u64,
    pub previous_action: Action,
    pub action: Action,
    pub reason: DecisionReason,
    pub navigation: NavigationOutcome,
    pub directive: Option<Directive>,
    pub destination: Option<Point>,
    pub known_contacts: usize,
    pub known_threat: Option<(SoldierId, Point, f64)>,
    pub sampled_morale: f64,
    pub sampled_suppression: f64,
    pub sampled_ammunition: u32,
}
#[derive(Default)]
pub(crate) struct DecisionLog {
    pub enabled: bool,
    pub records: BTreeMap<SoldierId, VecDeque<DecisionRecord>>,
}
impl DecisionLog {
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
    pub fn record(
        &mut self,
        tick: u64,
        before: &Soldier,
        after: &Soldier,
        reason: DecisionReason,
        navigation: NavigationOutcome,
        form: Option<&Formation>,
    ) {
        if !self.enabled {
            return;
        }
        let threat = before
            .contacts
            .iter()
            .min_by(|a, b| {
                before
                    .position
                    .distance(a.position)
                    .total_cmp(&before.position.distance(b.position))
                    .then(a.enemy.cmp(&b.enemy))
            })
            .map(|c| (c.enemy, c.position, before.position.distance(c.position)));
        let mut record = DecisionRecord {
            tick,
            entered_at: tick,
            previous_action: before.action.clone(),
            action: after.action.clone(),
            reason,
            navigation,
            directive: form.map(|f| f.directive),
            destination: after.destination,
            known_contacts: before.contacts.len(),
            known_threat: threat,
            sampled_morale: before.morale,
            sampled_suppression: before.suppression,
            sampled_ammunition: before.inventory.rounds,
        };
        let history = self.records.entry(after.id).or_default();
        if let Some(last) = history.back_mut()
            && last.action == record.action
            && last.reason == reason
            && last.navigation == navigation
            && last.directive == record.directive
        {
            record.entered_at = last.entered_at;
            record.previous_action = last.previous_action.clone();
            *last = record;
        } else {
            history.push_back(record);
            while history.len() > DECISION_HISTORY {
                history.pop_front();
            }
        }
    }
}
