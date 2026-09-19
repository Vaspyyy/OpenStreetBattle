use crate::SimError;
use bevy_ecs::prelude::Component;
use osb_content::Content;
use osb_planner::{Intent, Objective};
use osb_world::{Map, Navigation, Point};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const TICKS_PER_SECOND: u64 = 10;
pub const DT: f64 = 1.0 / TICKS_PER_SECOND as f64;
pub const SNAPSHOT_VERSION: u32 = 1;
pub const EVENT_LIMIT: usize = 2048;
pub const VISION_RANGE: f64 = 280.0;

macro_rules! id {
    ($name:ident,$ty:ty) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            Default,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub $ty);
    };
}
id!(SoldierId, u64);
id!(FormationId, u64);
id!(FactionId, u16);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForceSpec {
    pub faction: FactionId,
    pub name: String,
    pub soldiers: u32,
    pub spawn: Point,
    pub intent: Intent,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub schema_version: u32,
    pub name: String,
    pub map: Map,
    pub forces: Vec<ForceSpec>,
}
impl Scenario {
    pub fn demo() -> Self {
        let target = Point::new(550.0, 300.0);
        Self {
            schema_version: 1,
            name: "First Contact".into(),
            map: Map::demo(),
            forces: vec![
                ForceSpec {
                    faction: FactionId(1),
                    name: "Blue".into(),
                    soldiers: 20,
                    spawn: Point::new(80.0, 290.0),
                    intent: Intent {
                        objective: Objective::Capture,
                        target,
                        radius_m: 45.0,
                        casualty_tolerance: 0.55,
                    },
                },
                ForceSpec {
                    faction: FactionId(2),
                    name: "Red".into(),
                    soldiers: 12,
                    spawn: Point::new(790.0, 290.0),
                    intent: Intent {
                        objective: Objective::Defend,
                        target,
                        radius_m: 45.0,
                        casualty_tolerance: 0.55,
                    },
                },
            ],
        }
    }
    pub fn on_map(map: Map) -> Result<Self, SimError> {
        let nav = Navigation::new(&map, 10.0)?;
        let b = map.bounds;
        let point = |fraction: f64| {
            nav.nearest_walkable(Point::new(
                b.min.x + (b.max.x - b.min.x) * fraction,
                b.center().y,
            ))
            .ok_or_else(|| SimError::Invalid("map contains no walkable space".into()))
        };
        let mut s = Self::demo();
        s.forces[0].spawn = point(0.15)?;
        s.forces[1].spawn = point(0.85)?;
        let target = point(0.6)?;
        for f in &mut s.forces {
            f.intent.target = target;
        }
        s.name = map.name.clone();
        s.map = map;
        s.validate()?;
        Ok(s)
    }
    pub fn validate(&self) -> Result<(), SimError> {
        if self.schema_version != 1 || self.forces.is_empty() || self.forces.len() > 8 {
            return Err(SimError::Invalid(
                "unsupported scenario version or force count".into(),
            ));
        }
        self.map.validate()?;
        let mut ids = BTreeSet::new();
        let mut total = 0u64;
        for f in &self.forces {
            if !ids.insert(f.faction)
                || f.soldiers == 0
                || f.soldiers > 100_000
                || !self.map.walkable(f.spawn)
            {
                return Err(SimError::Invalid(format!("invalid force {}", f.name)));
            }
            f.intent.validate(&self.map)?;
            total += u64::from(f.soldiers);
        }
        if total > 100_000 {
            return Err(SimError::Invalid("scenario exceeds import safety limit of 100,000 people; not a performance guarantee".into()));
        }
        Ok(())
    }
    pub fn parse(text: &str) -> Result<Self, SimError> {
        if text.len() > 64 * 1024 * 1024 {
            return Err(SimError::Invalid("scenario exceeds 64 MiB".into()));
        }
        let s: Self = serde_json::from_str(text)?;
        s.validate()?;
        Ok(s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyRegion {
    Head,
    Torso,
    Arms,
    Legs,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Injury {
    pub region: BodyRegion,
    pub severity: f64,
    pub bleeding_per_second: f64,
    pub fracture: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Health {
    pub blood: f64,
    pub pain: f64,
    pub shock: f64,
    pub dead: bool,
    pub injuries: Vec<Injury>,
}
impl Default for Health {
    fn default() -> Self {
        Self {
            blood: 1.0,
            pain: 0.0,
            shock: 0.0,
            dead: false,
            injuries: Vec::new(),
        }
    }
}
impl Health {
    pub fn bleeding(&self) -> f64 {
        self.injuries.iter().map(|i| i.bleeding_per_second).sum()
    }
    pub fn conscious(&self) -> bool {
        !self.dead && self.blood > 0.45 && self.shock < 0.9
    }
    pub fn mobility(&self) -> f64 {
        (1.0 - self
            .injuries
            .iter()
            .filter(|i| i.region == BodyRegion::Legs)
            .map(|i| i.severity * 0.8)
            .sum::<f64>())
        .clamp(0.05, 1.0)
    }
    pub fn handling(&self) -> f64 {
        (1.0 - self
            .injuries
            .iter()
            .filter(|i| i.region == BodyRegion::Arms)
            .map(|i| i.severity * 0.8)
            .sum::<f64>())
        .clamp(0.05, 1.0)
    }
    pub fn tick(&mut self) {
        if self.dead {
            return;
        }
        self.blood = (self.blood - self.bleeding() * DT).max(0.0);
        self.shock = (self.pain * 0.3 + (1.0 - self.blood) * 0.9).clamp(0.0, 1.0);
        if self.blood <= 0.15 {
            self.dead = true;
        }
    }
    pub fn injure(&mut self, region: BodyRegion, severity: f64) {
        let severity = severity.clamp(0.0, 1.0);
        self.pain = (self.pain + severity * 0.5).min(1.0);
        self.blood = (self.blood - severity * 0.15).max(0.0);
        if (region == BodyRegion::Head && severity > 0.5)
            || (region == BodyRegion::Torso && severity > 0.85)
            || self.blood <= 0.15
        {
            self.dead = true;
        }
        self.injuries.push(Injury {
            region,
            severity,
            bleeding_per_second: severity * 0.003,
            fracture: severity > 0.5 && matches!(region, BodyRegion::Arms | BodyRegion::Legs),
        });
        self.shock = (self.pain * 0.3 + (1.0 - self.blood) * 0.9).clamp(0.0, 1.0);
    }
    /// Stabilization stops most bleeding but never restores blood or removes wounds.
    pub fn stabilize(&mut self) {
        for i in &mut self.injuries {
            i.bleeding_per_second *= 0.02;
        }
    }
    pub fn validate(&self) -> bool {
        [self.blood, self.pain, self.shock]
            .into_iter()
            .all(unit_interval)
            && self.injuries.len() <= 256
            && self.injuries.iter().all(|i| {
                unit_interval(i.severity)
                    && i.bleeding_per_second.is_finite()
                    && (0.0..=1.0).contains(&i.bleeding_per_second)
            })
    }
}
fn unit_interval(x: f64) -> bool {
    x.is_finite() && (0.0..=1.0).contains(&x)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub weapon: String,
    pub ammunition: String,
    pub rounds: u32,
    pub grenades: u16,
    pub medical_supplies: u16,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    pub other: SoldierId,
    pub trust: f64,
    pub attachment: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contact {
    pub enemy: SoldierId,
    pub position: Point,
    pub observed_tick: u64,
    pub observer: SoldierId,
    pub shared: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Idle,
    Moving,
    Holding,
    Engaging(SoldierId),
    TakingCover,
    Retreating,
    Assisting(SoldierId),
    Surrendered,
    Incapacitated,
    Dead,
}
#[derive(Clone, Debug, PartialEq, Component, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Soldier {
    pub id: SoldierId,
    pub name: String,
    pub faction: FactionId,
    pub position: Point,
    pub assigned_formation: FormationId,
    pub current_group: FormationId,
    pub rank: u8,
    pub leadership: f64,
    pub courage: f64,
    pub medic: bool,
    pub radio: bool,
    pub health: Health,
    pub morale: f64,
    pub suppression: f64,
    pub fatigue: f64,
    pub inventory: Inventory,
    pub relationships: Vec<Relationship>,
    pub contacts: Vec<Contact>,
    pub action: Action,
    pub action_since: u64,
    pub cooldown_until: u64,
    pub destination: Option<Point>,
    pub path: Vec<Point>,
    pub repath_at: u64,
}
impl Soldier {
    pub fn active(&self) -> bool {
        self.health.conscious() && self.action != Action::Surrendered
    }
    pub fn set_action(&mut self, action: Action, tick: u64) {
        if self.action != action {
            self.action = action;
            self.action_since = tick;
        }
    }
    pub fn attachment(&self, id: SoldierId) -> f64 {
        self.relationships
            .iter()
            .find(|r| r.other == id)
            .map_or(0.0, |r| r.attachment)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormationLevel {
    Platoon,
    Squad,
    Fireteam,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Directive {
    Advance,
    Hold,
    Engage,
    Regroup,
    Withdraw,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Formation {
    pub id: FormationId,
    pub parent: Option<FormationId>,
    pub faction: FactionId,
    pub name: String,
    pub level: FormationLevel,
    pub leader: Option<SoldierId>,
    pub nominal_strength: u32,
    pub intent: Intent,
    pub rally: Point,
    pub directive: Directive,
    pub target: Point,
    pub held_ticks: u64,
    pub secured: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Contact,
    Shot,
    Injury,
    Death,
    Incapacitated,
    Surrender,
    LeaderChanged,
    GroupJoined,
    AmmoShared,
    Treatment,
    ObjectiveSecured,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimEvent {
    pub sequence: u64,
    pub tick: u64,
    pub kind: EventKind,
    pub actor: Option<SoldierId>,
    pub target: Option<SoldierId>,
    pub position: Option<Point>,
    pub description: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema_version: u32,
    pub tick: u64,
    pub seed: u64,
    pub rng_state: u64,
    pub next_event: u64,
    pub scenario: Scenario,
    pub content: Content,
    pub soldiers: Vec<Soldier>,
    pub formations: Vec<Formation>,
    pub recent_events: VecDeque<SimEvent>,
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), SimError> {
        if self.schema_version != SNAPSHOT_VERSION {
            return Err(SimError::Invalid(format!(
                "unsupported snapshot version {}",
                self.schema_version
            )));
        }
        self.scenario.validate()?;
        self.content.validate()?;
        if self.tick > u64::MAX - 10_000
            || self.next_event == 0
            || self.recent_events.len() > EVENT_LIMIT
            || self.soldiers.len() > 100_000
        {
            return Err(SimError::Invalid(
                "invalid clock, event buffer or roster size".into(),
            ));
        }
        let mut ids = BTreeMap::new();
        let mut forms = BTreeMap::new();
        for s in &self.soldiers {
            if s.id.0 == 0 || ids.insert(s.id, s).is_some() {
                return Err(SimError::Invalid("duplicate soldier identity".into()));
            }
        }
        for f in &self.formations {
            if f.id.0 == 0 || forms.insert(f.id, f).is_some() {
                return Err(SimError::Invalid("duplicate formation identity".into()));
            }
        }
        for f in &self.formations {
            f.intent.validate(&self.scenario.map)?;
            if !self
                .scenario
                .forces
                .iter()
                .any(|force| force.faction == f.faction)
                || !self.scenario.map.walkable(f.rally)
                || !self.scenario.map.walkable(f.target)
            {
                return Err(SimError::Invalid(
                    "invalid formation faction or position".into(),
                ));
            }
            if f.leader
                .is_some_and(|id| ids.get(&id).is_none_or(|s| s.faction != f.faction))
            {
                return Err(SimError::Invalid(
                    "dangling or hostile formation leader".into(),
                ));
            }
            let mut current = f.parent;
            let mut seen = BTreeSet::from([f.id]);
            while let Some(id) = current {
                if !seen.insert(id) {
                    return Err(SimError::Invalid("cyclic command hierarchy".into()));
                }
                let p = forms
                    .get(&id)
                    .ok_or_else(|| SimError::Invalid("dangling formation parent".into()))?;
                if p.faction != f.faction {
                    return Err(SimError::Invalid("cross-faction hierarchy".into()));
                }
                current = p.parent;
            }
        }
        for s in &self.soldiers {
            if !self.scenario.map.walkable(s.position)
                || !s.health.validate()
                || ![s.morale, s.suppression, s.fatigue, s.leadership, s.courage]
                    .into_iter()
                    .all(unit_interval)
            {
                return Err(SimError::Invalid(format!(
                    "invalid soldier state {}",
                    s.id.0
                )));
            }
            for id in [s.assigned_formation, s.current_group] {
                if forms.get(&id).is_none_or(|f| f.faction != s.faction) {
                    return Err(SimError::Invalid("dangling or hostile membership".into()));
                }
            }
            let Some(w) = self.content.weapon(&s.inventory.weapon) else {
                return Err(SimError::Invalid("unknown weapon reference".into()));
            };
            if w.ammunition != s.inventory.ammunition || s.inventory.rounds > 1_000_000 {
                return Err(SimError::Invalid("incompatible inventory".into()));
            }
            for r in &s.relationships {
                if r.other == s.id
                    || ids
                        .get(&r.other)
                        .is_none_or(|other| other.faction != s.faction)
                    || !unit_interval(r.attachment)
                    || !unit_interval(r.trust)
                {
                    return Err(SimError::Invalid("invalid relationship".into()));
                }
            }
            for c in &s.contacts {
                if ids.get(&c.enemy).is_none_or(|e| e.faction == s.faction)
                    || !ids.contains_key(&c.observer)
                    || !c.position.finite()
                    || c.observed_tick > self.tick
                {
                    return Err(SimError::Invalid("invalid contact".into()));
                }
            }
            if s.path.iter().any(|&p| !self.scenario.map.walkable(p))
                || s.destination
                    .is_some_and(|p| !self.scenario.map.walkable(p))
            {
                return Err(SimError::Invalid("invalid navigation state".into()));
            }
            match s.action {
                Action::Engaging(id) | Action::Assisting(id) if !ids.contains_key(&id) => {
                    return Err(SimError::Invalid("dangling action target".into()));
                }
                _ => {}
            }
        }
        let mut previous = 0;
        for e in &self.recent_events {
            if e.sequence <= previous
                || e.sequence >= self.next_event
                || e.tick > self.tick
                || e.actor.is_some_and(|id| !ids.contains_key(&id))
                || e.target.is_some_and(|id| !ids.contains_key(&id))
            {
                return Err(SimError::Invalid("invalid event history".into()));
            }
            previous = e.sequence;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ForceStats {
    pub faction: FactionId,
    pub deployed: usize,
    pub effective: usize,
    pub wounded: usize,
    pub incapacitated: usize,
    pub dead: usize,
    pub surrendered: usize,
    pub rounds: u64,
}
