//! Fixed-step reference simulation shared by the native observer and headless runner.
//! No wall clock, camera, network, rendering or nondeterministic parallel scheduling here.
mod model;
mod spatial;
pub use model::*;
use bevy_ecs::prelude::{Entity,World};
use osb_content::Content;
use osb_planner::Objective;
use osb_world::{Navigation,Point};
use spatial::Spatial;
use std::collections::{BTreeMap,BTreeSet,VecDeque};

#[derive(Debug,thiserror::Error)]
pub enum SimError {
    #[error("world: {0}")] World(#[from]osb_world::WorldError),
    #[error("content: {0}")] Content(#[from]osb_content::ContentError),
    #[error("intent: {0}")] Intent(#[from]osb_planner::IntentError),
    #[error("JSON: {0}")] Json(#[from]serde_json::Error),
    #[error("invalid simulation: {0}")] Invalid(String),
}

pub struct Simulation {
    world:World,index:BTreeMap<SoldierId,Entity>,navigation:Navigation,
    scenario:Scenario,content:Content,formations:Vec<Formation>,events:VecDeque<SimEvent>,
    tick:u64,seed:u64,rng:u64,next_event:u64,
}
/// Explicit SplitMix64 state, serialized in snapshots. Not cryptographic randomness.
fn random(state:&mut u64)->u64 {
    *state=state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z=*state;z=(z^(z>>30)).wrapping_mul(0xbf58476d1ce4e5b9);z=(z^(z>>27)).wrapping_mul(0x94d049bb133111eb);z^(z>>31)
}
fn fraction(state:&mut u64)->f64 {(random(state)>>11) as f64/(1u64<<53) as f64}

impl Simulation {
    pub fn new(scenario:Scenario,seed:u64)->Result<Self,SimError> {Self::with_content(scenario,Content::generic(),seed)}
    pub fn with_content(scenario:Scenario,content:Content,seed:u64)->Result<Self,SimError> {
        scenario.validate()?;content.validate()?;
        if content.weapon("service_rifle").is_none() || content.weapon("support_weapon").is_none() {return Err(SimError::Invalid("baseline loadout requires service_rifle and support_weapon content IDs".into()));}
        let navigation=Navigation::new(&scenario.map,10.0)?;
        let mut sim=Self{world:World::new(),index:BTreeMap::new(),navigation,scenario,content,formations:Vec::new(),events:VecDeque::new(),tick:0,seed,rng:seed,next_event:1};
        let mut next_soldier=1u64;let mut next_formation=1u64;
        for force in sim.scenario.forces.clone() {
            let root=FormationId(next_formation);next_formation+=1;
            sim.formations.push(Formation{id:root,parent:None,faction:force.faction,name:format!("{} Platoon",force.name),level:FormationLevel::Platoon,leader:None,nominal_strength:force.soldiers,intent:force.intent.clone(),rally:force.spawn,directive:Directive::Advance,target:force.intent.target,held_ticks:0,secured:false});
            let first=next_soldier;
            let mut squad=root;let mut team=root;
            for i in 0..force.soldiers {
                if i%8==0 {squad=FormationId(next_formation);next_formation+=1;sim.formations.push(Formation{id:squad,parent:Some(root),faction:force.faction,name:format!("{} / Squad {}",force.name,i/8+1),level:FormationLevel::Squad,leader:None,nominal_strength:(force.soldiers-i).min(8),intent:force.intent.clone(),rally:force.spawn,directive:Directive::Advance,target:force.intent.target,held_ticks:0,secured:false});}
                if i%4==0 {team=FormationId(next_formation);next_formation+=1;sim.formations.push(Formation{id:team,parent:Some(squad),faction:force.faction,name:format!("{} / Team {}",force.name,i/4+1),level:FormationLevel::Fireteam,leader:None,nominal_strength:(force.soldiers-i).min(4),intent:force.intent.clone(),rally:force.spawn,directive:Directive::Advance,target:force.intent.target,held_ticks:0,secured:false});}
                let id=SoldierId(next_soldier);next_soldier+=1;
                let candidate=sim.scenario.map.bounds.clamp(Point::new(force.spawn.x+(i%4) as f64*5.0,force.spawn.y+(i/4) as f64*5.0));
                let position=if sim.scenario.map.walkable(candidate) {candidate} else {sim.navigation.nearest_walkable(candidate).ok_or_else(||SimError::Invalid("no walkable spawn".into()))?};
                let weapon=sim.content.weapon(if i%8==1 {"support_weapon"} else {"service_rifle"}).expect("validated loadouts");
                let buddy=i^1;
                let relationships=if buddy<force.soldiers {vec![Relationship{other:SoldierId(first+u64::from(buddy)),trust:0.8,attachment:0.85}]} else {Vec::new()};
                let soldier=Soldier{id,name:format!("{} {}",["Alex","Jonas","Robin","Sam","Leon","Morgan","Jules","Kai"][i as usize%8],id.0),faction:force.faction,position,assigned_formation:team,current_group:team,rank:if i==0 {4} else if i%8==0 {3} else if i%4==0 {2} else {1},leadership:0.4+fraction(&mut sim.rng)*0.5,courage:0.35+fraction(&mut sim.rng)*0.55,medic:i%8==7,radio:i%4==0,health:Health::default(),morale:0.8,suppression:0.0,fatigue:0.0,inventory:Inventory{weapon:weapon.id.clone(),ammunition:weapon.ammunition.clone(),rounds:weapon.starting_rounds,grenades:2,medical_supplies:if i%8==7 {6} else {1}},relationships,contacts:Vec::new(),action:Action::Idle,action_since:0,cooldown_until:0,destination:None,path:Vec::new(),repath_at:0};
                let entity=sim.world.spawn(soldier).id();sim.index.insert(id,entity);
            }
        }
        let roster=sim.soldiers();sim.update_command(&roster);
        sim.snapshot().validate()?;Ok(sim)
    }
    pub fn scenario(&self)->&Scenario {&self.scenario}
    pub fn tick(&self)->u64 {self.tick}
    pub fn formations(&self)->&[Formation] {&self.formations}
    pub fn events(&self)->&VecDeque<SimEvent> {&self.events}
    pub fn inspect(&self,id:SoldierId)->Option<&Soldier> {self.index.get(&id).and_then(|&e|self.world.get::<Soldier>(e))}
    pub fn soldiers(&self)->Vec<Soldier> {self.index.values().filter_map(|&e|self.world.get::<Soldier>(e).cloned()).collect()}
    pub fn snapshot(&self)->Snapshot {Snapshot{schema_version:SNAPSHOT_VERSION,tick:self.tick,seed:self.seed,rng_state:self.rng,next_event:self.next_event,scenario:self.scenario.clone(),content:self.content.clone(),soldiers:self.soldiers(),formations:self.formations.clone(),recent_events:self.events.clone()}}
    pub fn restore(mut snapshot:Snapshot)->Result<Self,SimError> {
        snapshot.validate()?;
        snapshot.soldiers.sort_by_key(|s|s.id);snapshot.formations.sort_by_key(|f|f.id);
        let navigation=Navigation::new(&snapshot.scenario.map,10.0)?;
        let mut world=World::new();let mut index=BTreeMap::new();
        for s in snapshot.soldiers {let id=s.id;index.insert(id,world.spawn(s).id());}
        Ok(Self{world,index,navigation,scenario:snapshot.scenario,content:snapshot.content,formations:snapshot.formations,events:snapshot.recent_events,tick:snapshot.tick,seed:snapshot.seed,rng:snapshot.rng_state,next_event:snapshot.next_event})
    }
    pub fn fingerprint(&self)->Result<String,SimError> {Ok(blake3::hash(&serde_json::to_vec(&self.snapshot())?).to_hex().to_string())}
    pub fn advance(&mut self,ticks:u64)->Result<(),SimError> {for _ in 0..ticks {self.step()?;}Ok(())}
    fn emit(&mut self,kind:EventKind,actor:Option<SoldierId>,target:Option<SoldierId>,position:Option<Point>,description:String) {
        self.events.push_back(SimEvent{sequence:self.next_event,tick:self.tick,kind,actor,target,position,description});self.next_event+=1;
        while self.events.len()>EVENT_LIMIT {self.events.pop_front();}
    }
    fn belongs(&self,mut group:FormationId,parent:FormationId)->bool {
        for _ in 0..=self.formations.len() {
            if group==parent {return true;}
            let Some(next)=self.formations.iter().find(|f|f.id==group).and_then(|f|f.parent) else {return false;};group=next;
        }
        false
    }
    fn update_command(&mut self,soldiers:&[Soldier]) {
        for i in 0..self.formations.len() {
            let f=&self.formations[i];
            let members:Vec<_>=soldiers.iter().filter(|s|s.active() && self.belongs(s.current_group,f.id)).collect();
            let leader=members.iter().max_by(|a,b|a.rank.cmp(&b.rank).then(a.leadership.total_cmp(&b.leadership)).then(b.id.cmp(&a.id))).map(|s|s.id);
            let has_contact=members.iter().any(|s|!s.contacts.is_empty());
            let remaining=members.len() as f64/f.nominal_strength.max(1) as f64;
            let directive=if remaining<1.0-f.intent.casualty_tolerance {Directive::Withdraw} else if has_contact {Directive::Engage} else if f.intent.objective==Objective::Defend && members.iter().any(|s|s.position.distance(f.intent.target)<f.intent.radius_m) {Directive::Hold} else {Directive::Advance};
            if leader!=f.leader {let name=f.name.clone();self.emit(EventKind::LeaderChanged,leader,None,None,format!("{name}: leadership changed"));}
            let f=&mut self.formations[i];f.leader=leader;f.directive=directive;f.target=if directive==Directive::Withdraw {f.rally} else {f.intent.target};
        }
    }
    fn perceive(&mut self,roster:&[Soldier],next:&mut [Soldier],spatial:&Spatial) {
        let mut direct=vec![Vec::<Contact>::new();roster.len()];
        for (i,s) in roster.iter().enumerate() {
            if !s.active() {continue;}
            for j in spatial.near(s.position,VISION_RANGE) {
                let e=&roster[j];
                if e.faction!=s.faction && e.active() && self.scenario.map.visible(s.position,e.position,VISION_RANGE) {
                    direct[i].push(Contact{enemy:e.id,position:e.position,observed_tick:self.tick,observer:s.id,shared:false});
                    if !s.contacts.iter().any(|c|c.enemy==e.id) {self.emit(EventKind::Contact,Some(s.id),Some(e.id),Some(e.position),format!("{} spotted {}",s.name,e.name));}
                }
            }
        }
        // One-hop sharing of actual observations only. No global enemy roster enters decisions.
        for (i,s) in roster.iter().enumerate() {
            let mut known:BTreeMap<SoldierId,Contact>=direct[i].iter().map(|c|(c.enemy,c.clone())).collect();
            if s.active() {
                for j in spatial.near(s.position,if s.radio {1000.0} else {55.0}) {
                    let ally=&roster[j];if ally.faction!=s.faction || !ally.active() {continue;}
                    let distance=s.position.distance(ally.position);
                    let local=distance<=55.0;
                    let radio=s.radio && ally.radio && distance<=1000.0;
                    if local || radio {for c in &direct[j] {known.entry(c.enemy).or_insert_with(||{let mut c=c.clone();c.shared=true;c});}}
                }
            }
            next[i].contacts=known.into_values().collect();
        }
    }
    fn move_agent(&self,s:&mut Soldier,destination:Point,speed:f64) {
        if !self.scenario.map.walkable(destination) {return;}
        if s.destination.is_none_or(|d|d.distance(destination)>4.0) {s.destination=Some(destination);s.path.clear();s.repath_at=0;}
        if s.path.is_empty() && s.position.distance(destination)>0.5 && self.tick>=s.repath_at {
            s.path=self.navigation.path(&self.scenario.map,s.position,destination);s.repath_at=self.tick+20;
        }
        let mut remaining=speed*s.health.mobility()*(1.0-s.fatigue*0.4)*DT;
        while let Some(&waypoint)=s.path.first() {
            let distance=s.position.distance(waypoint);
            let p=s.position.toward(waypoint,remaining);
            if !self.scenario.map.clear_segment(s.position,p) {s.path.clear();break;}
            s.position=p;
            if distance<=remaining {s.path.remove(0);remaining-=distance;} else {break;}
            if remaining<=0.0 {break;}
        }
        s.fatigue=(s.fatigue+0.0006).min(1.0);
    }
    fn cover_position(&self,s:&Soldier,threat:Point)->Option<Point> {
        let mut candidates=Vec::new();
        for o in &self.scenario.map.obstacles {for v in &o.vertices {for (dx,dy) in [(-3.0,-3.0),(-3.0,3.0),(3.0,-3.0),(3.0,3.0)] {
            let p=Point::new(v.x+dx,v.y+dy);
            if s.position.distance(p)<65.0 && self.scenario.map.walkable(p) && !self.scenario.map.clear_segment(threat,p) {candidates.push(p);}
        }}}
        candidates.sort_by(|a,b|s.position.distance(*a).total_cmp(&s.position.distance(*b)).then(a.x.total_cmp(&b.x)).then(a.y.total_cmp(&b.y)));
        candidates.into_iter().find(|&p|!self.navigation.path(&self.scenario.map,s.position,p).is_empty())
    }
    pub fn step(&mut self)->Result<(),SimError> {
        if self.tick>=u64::MAX-10_000 || self.next_event>u64::MAX-1_000_000 {return Err(SimError::Invalid("clock or event counter exhausted".into()));}
        self.tick+=1;
        let before=self.soldiers();let mut next=before.clone();
        for s in &mut next {
            s.health.tick();s.suppression=(s.suppression-0.014).max(0.0);
            if s.suppression<0.1 {s.morale=(s.morale+0.0002).min(0.9);s.fatigue=(s.fatigue-0.0002).max(0.0);}
        }
        let sensed=next.clone();let spatial=Spatial::new(&sensed);
        self.perceive(&sensed,&mut next,&spatial);
        if self.tick%10==0 {
            // Isolated survivors can attach to a physically nearby friendly group. No teleporting.
            let groups=next.clone();
            for s in &mut next {
                if !s.active() {continue;}
                let count=groups.iter().filter(|g|g.active() && g.current_group==s.current_group).count();
                if count>=2 {continue;}
                let candidate=spatial.near(s.position,60.0).into_iter().map(|i|&groups[i]).filter(|g|g.active() && g.faction==s.faction && g.current_group!=s.current_group && s.position.distance(g.position)<=60.0 && self.scenario.map.clear_segment(s.position,g.position)).min_by(|a,b|s.position.distance(a.position).total_cmp(&s.position.distance(b.position)).then(a.id.cmp(&b.id)));
                if let Some(ally)=candidate {s.current_group=ally.current_group;self.emit(EventKind::GroupJoined,Some(s.id),Some(ally.id),Some(s.position),format!("{} joined a nearby team",s.name));}
            }
            self.update_command(&next);
        }
        let observed=next.clone();
        let lookup:BTreeMap<SoldierId,usize>=observed.iter().enumerate().map(|(i,s)|(s.id,i)).collect();
        let mut claimed_patients=BTreeSet::new();let mut treatments=Vec::new();
        let mut shots=Vec::new();
        for i in 0..next.len() {
            let old=&observed[i];let s=&mut next[i];
            if s.health.dead {s.set_action(Action::Dead,self.tick);continue;}
            if !s.health.conscious() {s.set_action(Action::Incapacitated,self.tick);continue;}
            if s.action==Action::Surrendered {continue;}
            let nearest=old.contacts.iter().min_by(|a,b|old.position.distance(a.position).total_cmp(&old.position.distance(b.position)).then(a.enemy.cmp(&b.enemy)));
            let friend_count=spatial.near(s.position,60.0).into_iter().filter(|&j|j!=i && observed[j].active() && observed[j].faction==s.faction && s.position.distance(observed[j].position)<=60.0).count();
            let form=self.formations.iter().find(|f|f.id==s.current_group).expect("validated group");
            if s.morale<0.08 && friend_count==0 && nearest.is_some_and(|e|s.position.distance(e.position)<45.0) {
                s.set_action(Action::Surrendered,self.tick);self.emit(EventKind::Surrender,Some(s.id),None,Some(s.position),format!("{} surrendered",s.name));continue;
            }
            if s.morale<0.22 || form.directive==Directive::Withdraw {
                s.set_action(Action::Retreating,self.tick);self.move_agent(s,form.rally,2.8);continue;
            }
            let patient=if s.inventory.medical_supplies>0 && s.suppression<0.65 {
                spatial.near(s.position,65.0).into_iter().filter(|&j| {
                    let p=&observed[j];p.faction==s.faction && !p.health.dead && p.health.bleeding()>0.00005 && !claimed_patients.contains(&p.id) && s.position.distance(p.position)<=65.0 && self.scenario.map.clear_segment(s.position,p.position) && (j==i || s.medic || s.attachment(p.id)>0.7 && s.courage>0.45)
                }).min_by(|&a,&b| {
                    let score=|j:usize| observed[j].health.bleeding()*(1.0+s.attachment(observed[j].id))/(1.0+s.position.distance(observed[j].position));
                    score(b).total_cmp(&score(a)).then(a.cmp(&b))
                })
            } else {None};
            if let Some(j)=patient {
                let p=&observed[j];claimed_patients.insert(p.id);s.set_action(Action::Assisting(p.id),self.tick);
                if s.position.distance(p.position)>2.0 {self.move_agent(s,p.position,1.8);}
                else if self.tick-s.action_since>=30 {s.inventory.medical_supplies-=1;treatments.push((i,j));s.action_since=self.tick;}
                continue;
            }
            if let Some(contact)=nearest {
                if s.suppression>0.6 {
                    s.set_action(Action::TakingCover,self.tick);
                    if s.path.is_empty() && self.tick>=s.repath_at {if let Some(cover)=self.cover_position(s,contact.position) {s.destination=Some(cover);}}
                    if let Some(cover)=s.destination {self.move_agent(s,cover,1.5);}
                    continue;
                }
                let weapon=self.content.weapon(&s.inventory.weapon).expect("validated weapon");
                if s.inventory.rounds>0 && self.scenario.map.visible(s.position,contact.position,weapon.range_m) {
                    s.set_action(Action::Engaging(contact.enemy),self.tick);
                    if self.tick>=s.cooldown_until {s.inventory.rounds-=1;s.cooldown_until=self.tick+u64::from(weapon.cycle_ticks);shots.push((i,contact.enemy,contact.position,weapon.clone()));}
                    continue;
                }
            }
            let goal=form.target;
            let distance=s.position.distance(goal);
            if distance>form.intent.radius_m*0.45 {s.set_action(Action::Moving,self.tick);self.move_agent(s,goal,1.8);} else {s.set_action(Action::Holding,self.tick);s.path.clear();}
        }
        for (i,j) in treatments {
            if !next[j].health.dead {next[j].health.stabilize();self.emit(EventKind::Treatment,Some(next[i].id),Some(next[j].id),Some(next[j].position),format!("{} stabilized {}",next[i].name,next[j].name));}
        }
        for (i,target,aim,weapon) in shots {
            let Some(&j)=lookup.get(&target) else {continue;};
            let shooter=&observed[i];
            self.emit(EventKind::Shot,Some(shooter.id),Some(target),Some(shooter.position),format!("{} fired",shooter.name));
            if !next[j].active() {continue;}
            // Authoritative hit resolution may read actual geometry/state; AI target choice may not.
            if !self.scenario.map.clear_segment(shooter.position,next[j].position) {continue;}
            next[j].suppression=(next[j].suppression+weapon.suppression).min(1.0);
            next[j].morale=(next[j].morale-weapon.suppression*0.025).max(0.0);
            let distance=shooter.position.distance(aim);
            let posture=if matches!(observed[j].action,Action::TakingCover|Action::Retreating) {0.45} else {1.0};
            let p=(weapon.hit_probability*(1.0-distance/weapon.range_m*0.7).clamp(0.05,1.0)*(1.0-shooter.suppression*0.8)*shooter.health.handling()*posture/(1.0+next[j].position.distance(aim)*0.15)).clamp(0.0,1.0);
            if fraction(&mut self.rng)<p {
                let region=match random(&mut self.rng)%10 {0=>BodyRegion::Head,1..=5=>BodyRegion::Torso,6..=7=>BodyRegion::Arms,_=>BodyRegion::Legs};
                let severity=weapon.wound_severity*(0.7+fraction(&mut self.rng)*0.8);
                next[j].health.injure(region,severity);
                next[j].morale=(next[j].morale-0.12).max(0.0);
                self.emit(EventKind::Injury,Some(shooter.id),Some(target),Some(next[j].position),format!("{} wounded",next[j].name));
            }
        }
        if self.tick%20==0 {
            for i in 0..next.len() {
                if !next[i].active() || next[i].inventory.rounds>=5 {continue;}
                let donor=(0..next.len()).find(|&j| j!=i && next[j].active() && next[j].current_group==next[i].current_group && next[j].inventory.ammunition==next[i].inventory.ammunition && next[j].inventory.rounds>60 && next[i].position.distance(next[j].position)<8.0);
                if let Some(j)=donor {next[j].inventory.rounds-=30;next[i].inventory.rounds+=30;self.emit(EventKind::AmmoShared,Some(next[j].id),Some(next[i].id),Some(next[i].position),"30 compatible rounds transferred".into());}
            }
        }
        for i in 0..next.len() {
            let died=next[i].health.dead && !before[i].health.dead;
            let collapsed=!next[i].health.conscious() && before[i].health.conscious() && !died;
            if died || collapsed {
                let (id,name,position,faction)=(next[i].id,next[i].name.clone(),next[i].position,next[i].faction);
                self.emit(if died {EventKind::Death} else {EventKind::Incapacitated},Some(id),None,Some(position),format!("{name} {}",if died {"died"} else {"became incapacitated"}));
                for ally in &mut next {if ally.id!=id && ally.faction==faction && ally.active() && self.scenario.map.visible(ally.position,position,100.0) {ally.morale=(ally.morale-0.04-ally.attachment(id)*0.2).max(0.0);}}
            }
            if next[i].health.dead {next[i].set_action(Action::Dead,self.tick);} else if !next[i].health.conscious() {next[i].set_action(Action::Incapacitated,self.tick);}
        }
        let mut secured=Vec::new();
        for f in self.formations.iter_mut().filter(|f|f.level==FormationLevel::Platoon) {
            let friendly=next.iter().any(|s|s.active() && s.faction==f.faction && s.position.distance(f.intent.target)<=f.intent.radius_m);
            let hostile=next.iter().any(|s|s.active() && s.faction!=f.faction && s.position.distance(f.intent.target)<=f.intent.radius_m);
            if friendly && !hostile {f.held_ticks+=1;if f.held_ticks>=300 && !f.secured {f.secured=true;secured.push((f.leader,f.intent.target,f.name.clone()));}} else {f.held_ticks=0;f.secured=false;}
        }
        for (leader,position,name) in secured {self.emit(EventKind::ObjectiveSecured,leader,None,Some(position),format!("{name} held its objective uncontested for 30 seconds"));}
        for s in next {let e=self.index[&s.id];self.world.entity_mut(e).insert(s);}
        Ok(())
    }
    /// Mutually exclusive personnel categories; wounded means conscious walking wounded.
    pub fn statistics(&self)->Vec<ForceStats> {
        let mut out:BTreeMap<FactionId,ForceStats>=self.scenario.forces.iter().map(|f|(f.faction,ForceStats{faction:f.faction,..Default::default()})).collect();
        for s in self.soldiers() {
            let f=out.get_mut(&s.faction).expect("validated faction");f.deployed+=1;f.rounds+=u64::from(s.inventory.rounds);
            if s.health.dead {f.dead+=1;} else if s.action==Action::Surrendered {f.surrendered+=1;} else if !s.health.conscious() {f.incapacitated+=1;} else if !s.health.injuries.is_empty() {f.wounded+=1;} else {f.effective+=1;}
        }
        out.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn random_golden_vector(){let mut r=0;assert_eq!(random(&mut r),16294208416658607535);}
    #[test] fn stable_identity_count(){let mut s=Simulation::new(Scenario::demo(),42).unwrap();let ids:Vec<_>=s.soldiers().iter().map(|s|s.id).collect();s.advance(200).unwrap();assert_eq!(ids,s.soldiers().iter().map(|s|s.id).collect::<Vec<_>>());}
    #[test] fn seeded_runs_match(){let mut a=Simulation::new(Scenario::demo(),42).unwrap();let mut b=Simulation::new(Scenario::demo(),42).unwrap();a.advance(100).unwrap();b.advance(100).unwrap();assert_eq!(a.fingerprint().unwrap(),b.fingerprint().unwrap());}
    #[test] fn inspection_does_not_affect_outcome(){let mut a=Simulation::new(Scenario::demo(),9).unwrap();let mut b=Simulation::new(Scenario::demo(),9).unwrap();for _ in 0..50 {a.step().unwrap();let _=a.soldiers();let _=a.snapshot();}b.advance(50).unwrap();assert_eq!(a.fingerprint().unwrap(),b.fingerprint().unwrap());}
    #[test] fn json_resume_is_exact(){let mut a=Simulation::new(Scenario::demo(),7).unwrap();a.advance(70).unwrap();let text=serde_json::to_string(&a.snapshot()).unwrap();let mut b=Simulation::restore(serde_json::from_str(&text).unwrap()).unwrap();a.advance(90).unwrap();b.advance(90).unwrap();assert_eq!(a.fingerprint().unwrap(),b.fingerprint().unwrap());}
    #[test] fn casualty_categories_conserve_people(){let mut a=Simulation::new(Scenario::demo(),3).unwrap();a.advance(200).unwrap();for f in a.statistics() {assert_eq!(f.deployed,f.effective+f.wounded+f.incapacitated+f.dead+f.surrendered);}}
    #[test] fn no_contact_across_solid_building(){let mut scenario=Scenario::demo();scenario.forces[0].soldiers=1;scenario.forces[1].soldiers=1;scenario.forces[0].spawn=Point::new(380.0,300.0);scenario.forces[1].spawn=Point::new(520.0,300.0);let mut s=Simulation::new(scenario,42).unwrap();s.step().unwrap();assert!(s.soldiers().iter().all(|s|s.contacts.is_empty()));assert!(!s.events().iter().any(|e|e.kind==EventKind::Shot));}
    #[test] fn no_omniscient_contact_at_start(){let mut s=Simulation::new(Scenario::demo(),42).unwrap();s.step().unwrap();assert!(s.soldiers().iter().all(|s|s.contacts.is_empty()));}
    #[test] fn movement_is_bounded(){let mut s=Simulation::new(Scenario::demo(),1).unwrap();let before=s.soldiers();s.step().unwrap();for (a,b) in before.iter().zip(s.soldiers()) {assert!(a.position.distance(b.position)<=2.8*DT+1e-8);assert!(s.scenario().map.clear_segment(a.position,b.position));}}
    #[test] fn treatment_preserves_wounds(){let mut h=Health::default();h.injure(BodyRegion::Legs,0.6);let blood=h.blood;let mobility=h.mobility();let bleeding=h.bleeding();h.stabilize();assert_eq!(h.blood,blood);assert_eq!(h.mobility(),mobility);assert!(h.bleeding()<bleeding);assert_eq!(h.injuries.len(),1);}
    #[test] fn untreated_bleeding_has_consequences(){let mut h=Health::default();h.injure(BodyRegion::Legs,0.7);for _ in 0..5000 {h.tick();}assert!(h.dead);}
    #[test] fn duplicate_identity_rejected(){let s=Simulation::new(Scenario::demo(),42).unwrap();let mut snap=s.snapshot();snap.soldiers[1].id=snap.soldiers[0].id;assert!(Simulation::restore(snap).is_err());}
    #[test] fn hierarchy_cycles_rejected(){let s=Simulation::new(Scenario::demo(),42).unwrap();let mut snap=s.snapshot();snap.formations[0].parent=Some(snap.formations[0].id);assert!(Simulation::restore(snap).is_err());}
    #[test] fn missing_friend_rejected(){let s=Simulation::new(Scenario::demo(),42).unwrap();let mut snap=s.snapshot();snap.soldiers[0].relationships[0].other=SoldierId(999999);assert!(Simulation::restore(snap).is_err());}
    #[test] fn unsupported_save_version_rejected(){let s=Simulation::new(Scenario::demo(),42).unwrap();let mut snap=s.snapshot();snap.schema_version=999;assert!(Simulation::restore(snap).is_err());}
    #[test] fn corpse_is_not_despawned(){let s=Simulation::new(Scenario::demo(),42).unwrap();let mut snap=s.snapshot();let id=snap.soldiers[0].id;snap.soldiers[0].health.dead=true;let mut restored=Simulation::restore(snap).unwrap();restored.step().unwrap();assert_eq!(restored.soldiers().len(),32);assert_eq!(restored.inspect(id).unwrap().action,Action::Dead);}
    #[test] fn successor_replaces_dead_leader(){let s=Simulation::new(Scenario::demo(),42).unwrap();let mut snap=s.snapshot();let team=snap.soldiers[0].current_group;let old=snap.formations.iter().find(|f|f.id==team).unwrap().leader;snap.soldiers.iter_mut().find(|s|Some(s.id)==old).unwrap().health.dead=true;let mut s=Simulation::restore(snap).unwrap();s.advance(10).unwrap();let new=s.formations().iter().find(|f|f.id==team).unwrap().leader;assert!(new.is_some());assert_ne!(old,new);}
}
