use super::{ClientState,EditMode};
use bevy::prelude::*;
use bevy_egui::{EguiContexts,egui};
use osb_sim::{Action,EventKind,FactionId,FormationLevel,Soldier,SoldierId,TICKS_PER_SECOND,VISION_RANGE};
use osb_world::Point;
use std::collections::BTreeMap;

const BLUE:egui::Color32=egui::Color32::from_rgb(94,177,242);
const RED:egui::Color32=egui::Color32::from_rgb(232,119,106);
const MUTED:egui::Color32=egui::Color32::from_rgb(139,157,164);
const ACCENT:egui::Color32=egui::Color32::from_rgb(155,208,171);
fn faction_color(f:FactionId)->egui::Color32 {if f.0==1 {BLUE} else {RED}}
fn point(p:egui::Pos2)->Point {Point::new(f64::from(p.x),f64::from(p.y))}
fn pos(p:Point)->egui::Pos2 {egui::pos2(p.x as f32,p.y as f32)}
fn clock(tick:u64)->String {let sec=tick/TICKS_PER_SECOND;format!("D{:03}  {:02}:{:02}:{:02}",sec/86400,(sec/3600)%24,(sec/60)%60,sec%60)}

pub fn draw(mut contexts:EguiContexts,mut state:NonSendMut<ClientState>)->Result {
    let ctx=contexts.ctx_mut()?;
    if state.frame<3 {
        let mut style=(*ctx.style()).clone();
        style.visuals=egui::Visuals::dark();
        style.visuals.panel_fill=egui::Color32::from_rgb(21,28,33);
        style.visuals.window_fill=egui::Color32::from_rgb(26,34,40);
        style.spacing.item_spacing=egui::vec2(9.0,7.0);
        ctx.set_style(style);
    }
    if !ctx.wants_keyboard_input() {
        if ctx.input(|i|i.key_pressed(egui::Key::Space)) {state.paused=!state.paused;}
        if ctx.input(|i|i.key_pressed(egui::Key::N)) && state.paused {state.step();}
        if ctx.input(|i|i.key_pressed(egui::Key::F5)) {state.save();}
        if ctx.input(|i|i.key_pressed(egui::Key::F9)) {state.load_checkpoint();}
        for (key,speed) in [(egui::Key::Num1,1.0),(egui::Key::Num2,2.0),(egui::Key::Num3,5.0),(egui::Key::Num4,10.0)] {if ctx.input(|i|i.key_pressed(key)) {state.speed=speed;state.max_speed=false;}}
        if ctx.input(|i|i.key_pressed(egui::Key::Num5)) {state.max_speed=true;}
    }
    egui::TopBottomPanel::top("header").min_height(58.0).show(ctx,|ui| {
        ui.horizontal_centered(|ui| {
            ui.heading(egui::RichText::new("OPENSTREETBATTLE").size(21.0).color(egui::Color32::WHITE));
            ui.label(egui::RichText::new("FOUNDATION  /  0.1").small().color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                ui.monospace(egui::RichText::new(clock(state.sim.tick())).size(17.0).color(ACCENT));
                ui.label(if state.paused {"PAUSED"} else {"OBSERVING"});
            });
        });
    });
    egui::TopBottomPanel::bottom("events").resizable(true).default_height(170.0).min_height(110.0).show(ctx,|ui| {
        ui.horizontal(|ui| {
            ui.strong("FIELD LOG");ui.label(egui::RichText::new("Direct simulation events, not generated narration").small().color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {ui.monospace(format!("tick {:.2} ms  |  {} persistent people",state.last_tick_ms,state.view.len()));});
        });
        ui.separator();
        let events:Vec<_>=state.sim.events().iter().rev().filter(|e|e.kind!=EventKind::Shot).take(80).cloned().collect();
        egui::ScrollArea::vertical().id_salt("event-scroll").max_height(90.0).show(ui,|ui| {
            for event in events {
                ui.horizontal(|ui| {
                    ui.monospace(egui::RichText::new(clock(event.tick)).color(MUTED));
                    if ui.selectable_label(false,&event.description).clicked() {if let Some(id)=event.actor {state.selected=Some(id);if let Some(s)=state.sim.inspect(id) {state.camera.center=s.position;}}}
                });
            }
        });
        ui.separator();ui.label(egui::RichText::new(&state.status).small().color(MUTED));
    });
    egui::SidePanel::left("setup").resizable(true).default_width(240.0).min_width(200.0).show(ctx,|ui| {
        egui::ScrollArea::vertical().show(ui,|ui| {
            ui.heading("Observe");ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.add_sized([110.0,32.0],egui::Button::new(if state.paused {"Play [Space]"} else {"Pause [Space]"})).clicked() {state.paused=!state.paused;}
                if ui.add_enabled(state.paused,egui::Button::new("Step [N]")).clicked() {state.step();}
            });
            ui.horizontal(|ui| {
                for speed in [1.0,2.0,5.0,10.0] {if ui.selectable_label(!state.max_speed && state.speed==speed,format!("{speed}x")).clicked() {state.speed=speed;state.max_speed=false;}}
                if ui.selectable_label(state.max_speed,"Max").clicked() {state.max_speed=true;}
            });
            ui.label(egui::RichText::new("Speed is best-effort. Simulation ticks are never skipped.").small().color(MUTED));
            ui.separator();
            ui.heading("Forces");
            for f in state.sim.statistics() {
                ui.group(|ui| {
                    ui.colored_label(faction_color(f.faction),format!("{} FORCE",if f.faction.0==1 {"BLUE"} else {"RED"}));
                    ui.label(format!("{} deployed / {} uninjured",f.deployed,f.effective));
                    ui.label(format!("{} wounded / {} incapacitated",f.wounded,f.incapacitated));
                    ui.label(format!("{} dead / {} surrendered",f.dead,f.surrendered));
                    ui.monospace(format!("{} rounds remaining",f.rounds));
                });
            }
            ui.separator();
            ui.collapsing("Scenario setup",|ui| {
                ui.label("Setup only. No tactical orders after Play.");
                let editable=state.sim.tick()==0;
                ui.add_enabled_ui(editable,|ui| {
                    let mut changed=false;
                    ui.horizontal(|ui| {ui.label("Seed");changed|=ui.add(egui::DragValue::new(&mut state.seed)).changed();});
                    for force in &mut state.draft.forces {
                        ui.horizontal(|ui| {ui.label(&force.name);changed|=ui.add(egui::DragValue::new(&mut force.soldiers).range(1..=500)).changed();});
                        changed|=ui.add(egui::Slider::new(&mut force.intent.casualty_tolerance,0.0..=1.0).text("loss tolerance")).changed();
                    }
                    if changed {state.rebuild();}
                    for (mode,label) in [(EditMode::Inspect,"Inspect"),(EditMode::BlueSpawn,"Place Blue"),(EditMode::RedSpawn,"Place Red"),(EditMode::BlueObjective,"Blue objective"),(EditMode::RedObjective,"Red objective")] {
                        ui.selectable_value(&mut state.mode,mode,label);
                    }
                    ui.label("Select a placement tool, then click open ground.");
                });
                if ui.button("Reset to edited scenario").clicked() {state.rebuild();}
                if ui.button("Export scenario.json").clicked() {
                    state.status=match serde_json::to_string_pretty(&state.draft).map_err(|e|e.to_string()).and_then(|s|std::fs::write("scenario.json",s).map_err(|e|e.to_string())) {Ok(())=>"Scenario exported to scenario.json in the working directory.".into(),Err(e)=>e};
                }
            });
            ui.collapsing("Map / geometry",|ui| {
                ui.label(&state.draft.map.name);
                ui.label(egui::RichText::new("Offline geometry backend. Live vector basemap is not integrated yet.").color(MUTED));
                ui.add_enabled_ui(state.sim.tick()==0,|ui| {ui.text_edit_singleline(&mut state.file_path);if ui.button("Import local OSM JSON").clicked() {state.import_map();}});
                for warning in &state.draft.map.warnings {ui.label(egui::RichText::new(warning).small().color(MUTED));}
            });
            ui.collapsing("Display",|ui| {
                ui.checkbox(&mut state.show_routes,"Planned paths");
                ui.checkbox(&mut state.show_groups,"Current team links");
                ui.checkbox(&mut state.show_contacts,"Selected soldier's contacts");
                if ui.button("Fit map").clicked() {state.camera.center=state.draft.map.bounds.center();state.camera.pixels_per_metre=0.8;state.follow=false;}
            });
            ui.separator();
            ui.horizontal(|ui| {if ui.button("Save [F5]").clicked() {state.save();}if ui.button("Load [F9]").clicked() {state.load_checkpoint();}});
            ui.label(egui::RichText::new("Manual checkpoints. Reset discards unsaved progress.").small().color(MUTED));
        });
    });
    egui::SidePanel::right("inspector").resizable(true).default_width(295.0).min_width(230.0).show(ctx,|ui| {
        egui::ScrollArea::vertical().show(ui,|ui| {
            ui.heading("Person inspector");
            let selected=state.selected.and_then(|id|state.sim.inspect(id).cloned());
            if let Some(s)=selected {inspect(ui,&s,&mut state);} else {ui.label("Click a person on the map.");}
        });
    });
    egui::CentralPanel::default().show(ctx,|ui| {map(ui,&mut state);});
    Ok(())
}

fn inspect(ui:&mut egui::Ui,s:&Soldier,state:&mut ClientState) {
    ui.add_space(6.0);
    ui.colored_label(faction_color(s.faction),egui::RichText::new(&s.name).size(20.0).strong());
    ui.monospace(format!("Persistent ID #{}",s.id.0));
    ui.label(format!("{:?}",s.action));
    ui.checkbox(&mut state.follow,"Follow this person");
    ui.separator();
    meter(ui,"Morale",s.morale);meter(ui,"Suppression",s.suppression);meter(ui,"Fatigue",s.fatigue);
    meter(ui,"Blood",s.health.blood);meter(ui,"Pain",s.health.pain);meter(ui,"Shock",s.health.shock);
    ui.label(format!("Mobility {:.0}% / handling {:.0}%",s.health.mobility()*100.0,s.health.handling()*100.0));
    ui.label(format!("Conscious: {} / injuries: {}",s.health.conscious(),s.health.injuries.len()));
    for injury in &s.health.injuries {ui.label(format!("{:?}: {:.0}% severity{}",injury.region,injury.severity*100.0,if injury.fracture {", fracture"} else {""}));}
    ui.separator();
    ui.strong("Equipment");ui.label(&s.inventory.weapon);
    ui.label(format!("{} rounds / {} medical items",s.inventory.rounds,s.inventory.medical_supplies));
    ui.label(egui::RichText::new(format!("{} grenades carried (use not implemented)",s.inventory.grenades)).small().color(MUTED));
    ui.label(format!("Role: {} / radio: {}",if s.medic {"medic"} else {"combatant"},s.radio));
    ui.separator();
    ui.strong("Organization");
    ui.label(format!("Assigned team #{}",s.assigned_formation.0));
    if let Some(f)=state.sim.formations().iter().find(|f|f.id==s.current_group) {
        ui.label(format!("Now: {}",f.name));ui.label(format!("Command: {:?}",f.directive));
        ui.label(format!("Leader: {}",f.leader.map_or("none".into(),|id|format!("#{}",id.0))));
    }
    ui.separator();ui.strong("Relationships");
    for r in &s.relationships {
        let label=state.sim.inspect(r.other).map_or(format!("#{}",r.other.0),|other|other.name.clone());
        if ui.link(format!("{} / bond {:.0}%",label,r.attachment*100.0)).clicked() {state.selected=Some(r.other);}
    }
    ui.separator();ui.strong(format!("Known contacts: {}",s.contacts.len()));
    for c in s.contacts.iter().take(12) {ui.label(format!("#{}: {} (observer #{})",c.enemy.0,if c.shared {"reported"} else {"personally seen"},c.observer.0));}
    ui.label(egui::RichText::new("Perception 1.0: current LOS plus one-hop sharing. Facing and stale reports come later.").small().color(MUTED));
}
fn meter(ui:&mut egui::Ui,label:&str,value:f64) {ui.add(egui::ProgressBar::new(value as f32).text(format!("{label} {:.0}%",value*100.0)));}

fn map(ui:&mut egui::Ui,state:&mut ClientState) {
    let (response,painter)=ui.allocate_painter(ui.available_size(),egui::Sense::click_and_drag());
    let rect=response.rect;let center=point(rect.center());
    if state.follow && let Some(s)=state.selected.and_then(|id|state.sim.inspect(id)) {state.camera.center=s.position;}
    if response.dragged() {
        let delta=ui.input(|i|i.pointer.delta());state.camera.pan_pixels(f64::from(delta.x),f64::from(delta.y));state.follow=false;
    }
    if response.hovered() && let Some(cursor)=ui.input(|i|i.pointer.hover_pos()) {
        let scroll=ui.input(|i|i.smooth_scroll_delta.y);
        if scroll!=0.0 {state.camera.zoom_at((f64::from(scroll)*0.0025).exp(),point(cursor),center);state.follow=false;}
    }
    if response.clicked() && let Some(cursor)=response.interact_pointer_pos() {
        let world=state.camera.to_world(point(cursor),center);
        if state.mode==EditMode::Inspect || state.sim.tick()>0 {
            state.selected=state.view.iter().min_by(|a,b|world.distance(state.display_position(a)).total_cmp(&world.distance(state.display_position(b)))).filter(|s|world.distance(state.display_position(s))*state.camera.pixels_per_metre<14.0).map(|s|s.id);
        } else if state.draft.map.walkable(world) {
            let index=if matches!(state.mode,EditMode::BlueSpawn|EditMode::BlueObjective) {0} else {1};
            if let Some(force)=state.draft.forces.get_mut(index) {
                if matches!(state.mode,EditMode::BlueSpawn|EditMode::RedSpawn) {force.spawn=world;} else {force.intent.target=world;}
                state.rebuild();
            }
        } else {state.status="Choose open ground inside the current map bounds.".into();}
    }
    let screen=|p:Point|pos(state.camera.to_screen(p,center));
    painter.rect_filled(rect,0.0,egui::Color32::from_rgb(30,42,43));
    let map=&state.draft.map;
    let grid=if state.camera.pixels_per_metre<0.1 {1000.0} else {100.0};
    let lower=state.camera.to_world(point(rect.left_bottom()),center);
    let upper=state.camera.to_world(point(rect.right_top()),center);
    for i in (lower.x/grid).floor() as i32..=(upper.x/grid).ceil() as i32 {
        let x=f64::from(i)*grid;painter.line_segment([screen(Point::new(x,lower.y)),screen(Point::new(x,upper.y))],egui::Stroke::new(1.0,egui::Color32::from_rgb(42,55,55)));
    }
    for i in (lower.y/grid).floor() as i32..=(upper.y/grid).ceil() as i32 {
        let y=f64::from(i)*grid;painter.line_segment([screen(Point::new(lower.x,y)),screen(Point::new(upper.x,y))],egui::Stroke::new(1.0,egui::Color32::from_rgb(42,55,55)));
    }
    for road in &map.roads {
        for segment in road.points.windows(2) {painter.line_segment([screen(segment[0]),screen(segment[1])],egui::Stroke::new((12.0*state.camera.pixels_per_metre as f32).clamp(1.5,24.0),egui::Color32::from_rgb(73,83,78)));}
        if state.camera.pixels_per_metre>0.55 && !road.name.is_empty() {painter.text(screen(road.points[road.points.len()/2])+egui::vec2(5.0,-12.0),egui::Align2::LEFT_CENTER,&road.name,egui::FontId::proportional(11.0),MUTED);}
    }
    for obstacle in &map.obstacles {
        // Outline rendering handles concave footprints without pretending they are convex.
        let mut vertices:Vec<_>=obstacle.vertices.iter().map(|&p|screen(p)).collect();
        if let Some(first)=vertices.first().copied() {vertices.push(first);}
        painter.add(egui::Shape::line(vertices,egui::Stroke::new(2.0,egui::Color32::from_rgb(120,130,117))));
        if obstacle.vertices.len()==4 {
            let pts:Vec<_>=obstacle.vertices.iter().map(|&p|screen(p)).collect();
            // The bundled rectangles are convex. Imported polygons are outlines only.
            if map.attribution.starts_with("Hand-authored") {painter.add(egui::Shape::convex_polygon(pts,egui::Color32::from_rgb(62,73,65),egui::Stroke::new(1.5,egui::Color32::from_rgb(120,130,117))));}
        }
    }
    for f in state.sim.formations().iter().filter(|f|f.level==FormationLevel::Platoon) {
        let p=screen(f.intent.target);let radius=(f.intent.radius_m*state.camera.pixels_per_metre) as f32;
        painter.circle_stroke(p,radius,egui::Stroke::new(1.5,faction_color(f.faction).gamma_multiply(0.65)));
        painter.text(p+egui::vec2(0.0,-radius-12.0),egui::Align2::CENTER_CENTER,format!("{:?} {}",f.intent.objective,if f.secured {"/ secured"} else {""}),egui::FontId::proportional(11.0),faction_color(f.faction));
    }
    if state.show_groups {
        let leaders:BTreeMap<_,_>=state.sim.formations().iter().filter_map(|f|f.leader.and_then(|id|state.sim.inspect(id)).map(|s|(f.id,s.position))).collect();
        for s in &state.view {if let Some(&p)=leaders.get(&s.current_group) {painter.line_segment([screen(s.position),screen(p)],egui::Stroke::new(1.0,faction_color(s.faction).gamma_multiply(0.18)));}}
    }
    if let Some(selected)=state.selected.and_then(|id|state.sim.inspect(id)) {
        let p=screen(state.display_position(selected));
        painter.circle_stroke(p,(VISION_RANGE*state.camera.pixels_per_metre) as f32,egui::Stroke::new(1.0,egui::Color32::from_rgba_unmultiplied(150,180,160,45)));
        if state.show_routes {
            let mut from=p;
            for &next in &selected.path {let to=screen(next);painter.line_segment([from,to],egui::Stroke::new(1.5,ACCENT.gamma_multiply(0.65)));from=to;}
        }
        if state.show_contacts {
            for c in &selected.contacts {let q=screen(c.position);painter.line_segment([p,q],egui::Stroke::new(1.0,if c.shared {egui::Color32::from_rgb(208,173,100)} else {RED.gamma_multiply(0.5)}));painter.circle_stroke(q,8.0,egui::Stroke::new(1.0,RED));}
        }
    }
    for s in &state.view {
        let p=screen(state.display_position(s));if !rect.expand(10.0).contains(p) {continue;}
        let selected=state.selected==Some(s.id);
        let color=if s.health.dead {egui::Color32::from_rgb(113,111,104)} else if !s.health.conscious() {egui::Color32::from_rgb(214,180,112)} else if s.action==Action::Surrendered {egui::Color32::WHITE} else {faction_color(s.faction)};
        let radius=(3.0*state.camera.pixels_per_metre as f32).clamp(3.0,7.0);
        if selected {painter.circle_stroke(p,radius+5.0,egui::Stroke::new(1.6,egui::Color32::WHITE));}
        if s.health.dead {painter.line_segment([p-egui::vec2(3.0,3.0),p+egui::vec2(3.0,3.0)],egui::Stroke::new(1.5,color));painter.line_segment([p-egui::vec2(3.0,-3.0),p+egui::vec2(3.0,-3.0)],egui::Stroke::new(1.5,color));}
        else {painter.circle_filled(p,radius,color);painter.circle_stroke(p,radius,egui::Stroke::new(1.0,egui::Color32::from_rgb(18,24,27)));}
        if s.medic && state.camera.pixels_per_metre>1.2 {painter.text(p+egui::vec2(9.0,-7.0),egui::Align2::LEFT_CENTER,"+",egui::FontId::monospace(12.0),ACCENT);}
    }
    let top=rect.left_top()+egui::vec2(16.0,18.0);
    painter.text(top,egui::Align2::LEFT_TOP,&map.name,egui::FontId::proportional(16.0),egui::Color32::WHITE);
    painter.text(top+egui::vec2(0.0,25.0),egui::Align2::LEFT_TOP,"OMNISCIENT OBSERVER  /  INDIVIDUAL AI HAS LIMITED KNOWLEDGE",egui::FontId::monospace(10.0),MUTED);
    painter.text(rect.left_bottom()+egui::vec2(12.0,-12.0),egui::Align2::LEFT_BOTTOM,&map.attribution,egui::FontId::proportional(10.0),MUTED);
    painter.text(rect.right_top()+egui::vec2(-14.0,20.0),egui::Align2::RIGHT_TOP,"N ↑",egui::FontId::proportional(16.0),ACCENT);
}
