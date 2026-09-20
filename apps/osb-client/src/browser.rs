//! World browsing is a separate, non-authoritative view. No camera reaches the simulation.
use crate::geographic::{self, PreparedBattle};
use bevy_egui::egui;
use osb_geodata::{Manifest, SnapshotStore};
use osb_map::GlobeCamera;
use osb_world::{GeoCoordinate, GeoRegion, Point};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const PUBLIC_ENDPOINT: &str = "https://overpass-api.de/api/interpreter";
const STYLE: &str = "https://tiles.openfreemap.org/styles/liberty";
struct Job {
    receiver: mpsc::Receiver<Result<PreparedBattle, String>>,
    cancel: Arc<AtomicBool>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
pub struct BrowserState {
    pub camera: GlobeCamera,
    selection: Option<GeoRegion>,
    drag_start: Option<GeoCoordinate>,
    selecting: bool,
    latitude: f64,
    longitude: f64,
    side_m: f64,
    endpoint: String,
    public_consent: bool,
    acknowledge: bool,
    basemap_consent: bool,
    status: String,
    job: Option<Job>,
    last_request: Option<Instant>,
    cache: Vec<Manifest>,
    prepared: Option<PreparedBattle>,
    software: bool,
    fixture: bool,
    pub basemap_ready: bool,
    #[cfg(feature = "live-map")]
    native: Option<osb_map_native::BasemapWorker>,
    #[cfg(feature = "live-map")]
    texture: Option<egui::TextureHandle>,
    #[cfg(feature = "live-map")]
    frame_view: Option<osb_map_native::View>,
    #[cfg(feature = "live-map")]
    requested_view: Option<osb_map_native::View>,
}
impl BrowserState {
    pub fn new(software: bool, fixture: bool) -> Self {
        let camera = if fixture {
            GlobeCamera {
                center: GeoCoordinate {
                    latitude: 54.005,
                    longitude: 10.005,
                },
                zoom: 12.0,
            }
        } else {
            GlobeCamera::default()
        };
        let mut s = Self {
            camera,
            selection: None,
            drag_start: None,
            selecting: false,
            latitude: camera.center.latitude,
            longitude: camera.center.longitude,
            side_m: 1000.0,
            endpoint: std::env::var("OSB_OVERPASS_ENDPOINT")
                .unwrap_or_else(|_| PUBLIC_ENDPOINT.into()),
            public_consent: false,
            acknowledge: false,
            basemap_consent: fixture,
            status: "Browse separately; your current battle stays paused and unchanged.".into(),
            job: None,
            last_request: None,
            cache: vec![],
            prepared: None,
            software,
            fixture,
            basemap_ready: false,
            #[cfg(feature = "live-map")]
            native: None,
            #[cfg(feature = "live-map")]
            texture: None,
            #[cfg(feature = "live-map")]
            frame_view: None,
            #[cfg(feature = "live-map")]
            requested_view: None,
        };
        s.refresh_cache();
        s
    }
    pub fn use_live_smoke(&mut self) {
        self.fixture = false;
        self.basemap_consent = true;
        self.camera.jump(
            GeoCoordinate {
                latitude: 52.52,
                longitude: 13.405,
            },
            14.0,
        );
        self.latitude = self.camera.center.latitude;
        self.longitude = self.camera.center.longitude;
    }
    fn refresh_cache(&mut self) {
        match SnapshotStore::new(geographic::cache_root()).list() {
            Ok(c) => self.cache = c,
            Err(e) => self.status = e.to_string(),
        }
    }
    fn poll_job(&mut self) {
        let received = self.job.as_ref().and_then(|j| match j.receiver.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("geographic worker stopped".into())),
            Err(mpsc::TryRecvError::Empty) => None,
        });
        if let Some(result) = received {
            let cancelled = self
                .job
                .as_ref()
                .is_some_and(|j| j.cancel.load(Ordering::Relaxed));
            self.job = None;
            if cancelled {
                self.status = "Cancelled. Existing battle retained.".into();
            } else {
                match result {
                    Ok(p) => {
                        self.status =
                            "Snapshot verified. Review limitations, then create the battle.".into();
                        self.prepared = Some(p);
                        self.acknowledge = false;
                    }
                    Err(e) => self.status = e,
                }
            }
            self.refresh_cache();
        }
    }
    fn fetch(&mut self, seed: u64) {
        if self.job.is_some() {
            return;
        }
        let Some(region) = self.selection else {
            return;
        };
        if self
            .last_request
            .is_some_and(|t| t.elapsed() < Duration::from_secs(5))
        {
            self.status = "Allow at least five seconds between manual requests.".into();
            return;
        }
        let endpoint = self.endpoint.trim().to_owned();
        if endpoint == PUBLIC_ENDPOINT && !self.public_consent {
            self.status = "Public development endpoint requires explicit consent.".into();
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        self.prepared = None;
        self.last_request = Some(Instant::now());
        self.status="Loading cached geography or requesting one bounded extract. Current battle is unchanged.".into();
        std::thread::spawn(move || {
            let result = geographic::acquire(
                &SnapshotStore::new(geographic::cache_root()),
                region,
                &endpoint,
                seed,
                &c,
            );
            let _ = sender.send(result);
        });
        self.job = Some(Job { receiver, cancel });
    }
    fn load_cached(&mut self, id: String, seed: u64) {
        if self.job.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.prepared = None;
        self.status =
            "Verifying cached source, geometry and provenance. No network request.".into();
        std::thread::spawn(move || {
            let result = SnapshotStore::new(geographic::cache_root())
                .load(&id)
                .map_err(|e| e.to_string())
                .and_then(|s| geographic::prepare(s, seed));
            let _ = sender.send(result);
        });
        self.job = Some(Job { receiver, cancel });
    }
    #[cfg(feature = "live-map")]
    fn stop_map(&mut self) {
        #[cfg(feature = "live-map")]
        {
            self.native = None;
            self.requested_view = None;
        }
        self.basemap_ready = false;
    }
    #[cfg(feature = "live-map")]
    fn render_map(&mut self, ui: &egui::Ui, rect: egui::Rect, painter: &egui::Painter) {
        if self.basemap_consent && self.native.is_none() {
            self.native = Some(osb_map_native::BasemapWorker::new(
                osb_map_native::Options {
                    cache_path: geographic::cache_root()
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .join("basemap/cache.db"),
                    style_url: std::env::var("OSB_MAP_STYLE").unwrap_or_else(|_| STYLE.into()),
                    software: self.software,
                    fixture: self.fixture,
                },
            ));
        }
        let shrink = (f64::from(rect.width()) / 2048.0)
            .max(f64::from(rect.height()) / 1536.0)
            .max(1.0);
        let view = osb_map_native::View {
            latitude: self.camera.center.latitude,
            longitude: self.camera.center.longitude,
            zoom: (self.camera.zoom - shrink.log2()).max(0.0),
            width: (f64::from(rect.width()) / shrink)
                .round()
                .clamp(32.0, 2048.0) as u32,
            height: (f64::from(rect.height()) / shrink)
                .round()
                .clamp(32.0, 1536.0) as u32,
        };
        if let Some(native) = &self.native {
            if self.requested_view != Some(view) {
                native.request(view);
                self.requested_view = Some(view);
                self.basemap_ready = false;
            }
            if let Some(result) = native.poll() {
                match result {
                    Ok(frame) => {
                        self.basemap_ready = frame.complete && frame.view == view;
                        let image = egui::ColorImage::from_rgba_premultiplied(
                            [frame.view.width as usize, frame.view.height as usize],
                            &frame.rgba,
                        );
                        if let Some(t) = &mut self.texture {
                            t.set(image, egui::TextureOptions::LINEAR);
                        } else {
                            self.texture = Some(ui.ctx().load_texture(
                                "world-basemap",
                                image,
                                egui::TextureOptions::LINEAR,
                            ));
                        }
                        self.frame_view = Some(frame.view);
                    }
                    Err(e) => {
                        self.status = format!("Basemap failed: {e}. Cached scenarios still work.");
                        self.basemap_consent = false;
                        self.stop_map();
                    }
                }
            }
        }
        if let (Some(texture), Some(frame)) = (&self.texture, self.frame_view) {
            // Reproject the most recent frame immediately while the native worker catches up.
            let scale = 2f64.powf(self.camera.zoom - frame.zoom) as f32;
            let center = self.camera.geo_to_screen(
                GeoCoordinate {
                    latitude: frame.latitude,
                    longitude: frame.longitude,
                },
                point(rect.center()),
            );
            let image_rect = egui::Rect::from_center_size(
                pos(center),
                egui::vec2(frame.width as f32 * scale, frame.height as f32 * scale),
            );
            painter.image(
                texture.id(),
                image_rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        if self.basemap_consent {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
    }
    #[cfg(not(feature = "live-map"))]
    fn render_map(&mut self, _ui: &egui::Ui, _rect: egui::Rect, _painter: &egui::Painter) {
        let _ = (&self.software, &self.fixture, STYLE);
    }
}
fn point(p: egui::Pos2) -> Point {
    Point::new(f64::from(p.x), f64::from(p.y))
}
fn pos(p: Point) -> egui::Pos2 {
    egui::pos2(p.x as f32, p.y as f32)
}
/// Returns a fully prepared, explicitly confirmed replacement. Never mutates the current battle.
pub fn draw(ui: &mut egui::Ui, state: &mut BrowserState, seed: u64) -> Option<PreparedBattle> {
    state.poll_job();
    let mut install = false;
    egui::Panel::left("world-tools").default_size(325.0).min_size(280.0).resizable(true).show(ui,|ui|{
        egui::ScrollArea::vertical().show(ui,|ui|{
            ui.heading("Real-world scenarios");
            ui.label("1. Browse  2. Select  3. Fetch  4. Review");ui.separator();
            #[cfg(feature="live-map")]
            if ui.checkbox(&mut state.basemap_consent,"Enable live OpenFreeMap basemap").changed() && !state.basemap_consent {state.stop_map();}
            #[cfg(not(feature="live-map"))]
            {ui.label("Native basemap is not compiled in this build.");ui.monospace("bash tools/run-live.sh");let _=state.basemap_consent;}
            ui.small("Enabling the basemap contacts the style/tile provider. It does not fetch simulation geometry.");
            ui.horizontal(|ui|{ui.label("Latitude");ui.add(egui::DragValue::new(&mut state.latitude).speed(0.001).range(-80.0..=80.0));});
            ui.horizontal(|ui|{ui.label("Longitude");ui.add(egui::DragValue::new(&mut state.longitude).speed(0.001).range(-180.0..=180.0));});
            ui.horizontal(|ui|{
                if ui.button("Go to coordinates").clicked(){state.camera.jump(GeoCoordinate {latitude:state.latitude,longitude:state.longitude},15.0);}
                if ui.button("Use view center").clicked(){state.latitude=state.camera.center.latitude;state.longitude=state.camera.center.longitude;}
            });
            ui.small("Pan with drag. Zoom with scroll. No public geocoder or autocomplete requests.");
            ui.separator();ui.heading("Battle area");
            ui.checkbox(&mut state.selecting,"Drag to select an area");
            ui.add(egui::Slider::new(&mut state.side_m,100.0..=2000.0).text("square side / m"));
            if ui.button("Select square at view center").clicked(){match GeoRegion::around(state.camera.center,state.side_m,state.side_m){Ok(r)=>state.selection=Some(r),Err(e)=>state.status=e.to_string()}}
            let valid=if let Some(region)=state.selection {
                let (w,h)=region.dimensions_m();ui.label(format!("{w:.0} × {h:.0} m / {:.2} km²",w*h/1e6));
                match region.validate_battle(){Ok(())=>true,Err(e)=>{ui.colored_label(egui::Color32::LIGHT_RED,e.to_string());false}}
            }else{ui.label("No area selected.");false};
            ui.separator();ui.label("OSM data endpoint");ui.text_edit_singleline(&mut state.endpoint);
            if state.endpoint.trim()==PUBLIC_ENDPOINT {ui.checkbox(&mut state.public_consent,"Occasional public development request");ui.small("One small extract on explicit click only. No bulk downloads, background prefetch or automatic retries. For sustained use configure your own/provider endpoint.");}
            let consent=state.endpoint.trim()!=PUBLIC_ENDPOINT || state.public_consent;
            if ui.add_enabled(valid && consent && state.job.is_none(),egui::Button::new("Fetch / reuse this area")).clicked(){state.fetch(seed);}
            if let Some(job)=&state.job {ui.spinner();if ui.button("Cancel request").clicked(){job.cancel.store(true,Ordering::Relaxed);state.status="Cancellation requested; the bounded HTTP operation may finish before stopping.".into();}ui.ctx().request_repaint_after(Duration::from_millis(100));}
            ui.separator();ui.collapsing("Cached regions (offline)",|ui|{
                if ui.button("Refresh cache list").clicked(){state.refresh_cache();}
                let mut chosen=None;
                for m in &state.cache {let c=m.region.center();if ui.button(format!("{:.4}, {:.4} / {}",c.latitude,c.longitude,&m.id[..8])).clicked(){chosen=Some(m.id.clone());}}
                if let Some(id)=chosen {state.load_cached(id,seed);}
                if state.cache.is_empty(){ui.label("No cached snapshots yet.");}
            });
            if let Some(p)=&state.prepared {
                ui.separator();ui.heading("Review snapshot");
                ui.label(format!("{} building parts / {} road parts",p.snapshot.map.obstacles.len(),p.snapshot.map.roads.len()));
                ui.monospace(format!("Snapshot {}",&p.snapshot.manifest.id[..12]));
                ui.label(format!("Acquired Unix time: {}",p.snapshot.manifest.acquired_unix_seconds));
                if let Some(t)=&p.snapshot.manifest.osm_base_timestamp {ui.label(format!("OSM data time: {t}"));}
                for warning in &p.snapshot.map.warnings {ui.small(warning);}
                ui.checkbox(&mut state.acknowledge,"I understand: incomplete 2D terrain; creating replaces the current unsaved battle.");
                if ui.add_enabled(state.acknowledge,egui::Button::new("Create battle from snapshot")).clicked(){install=true;}
            }
            ui.separator();ui.label(&state.status);
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = response.rect;
        let center = point(rect.center());
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(28, 39, 44));
        if response.drag_started()
            && state.selecting
            && let Some(p) = response.interact_pointer_pos()
        {
            state.drag_start = Some(state.camera.screen_to_geo(point(p), center));
        }
        if response.dragged() {
            if state.selecting {
                if let (Some(a), Some(p)) = (state.drag_start, response.interact_pointer_pos()) {
                    let b = state.camera.screen_to_geo(point(p), center);
                    state.selection = Some(GeoRegion {
                        south: a.latitude.min(b.latitude),
                        north: a.latitude.max(b.latitude),
                        west: a.longitude.min(b.longitude),
                        east: a.longitude.max(b.longitude),
                    });
                }
            } else {
                let d = ui.input(|i| i.pointer.delta());
                state.camera.pan(Point::new(f64::from(d.x), f64::from(d.y)));
            }
        }
        if response.drag_stopped() {
            state.drag_start = None;
        }
        if response.hovered()
            && let Some(p) = ui.input(|i| i.pointer.hover_pos())
        {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                state
                    .camera
                    .zoom_at(f64::from(scroll) * 0.004, point(p), center);
            }
        }
        state.render_map(ui, rect, &painter);
        if let Some(r) = state.selection {
            let a = pos(state.camera.geo_to_screen(
                GeoCoordinate {
                    latitude: r.north,
                    longitude: r.west,
                },
                center,
            ));
            let b = pos(state.camera.geo_to_screen(
                GeoCoordinate {
                    latitude: r.south,
                    longitude: r.east,
                },
                center,
            ));
            let selected = egui::Rect::from_two_pos(a, b);
            let color = if r.validate_battle().is_ok() {
                egui::Color32::from_rgb(122, 220, 160)
            } else {
                egui::Color32::LIGHT_RED
            };
            painter.rect_filled(selected, 0.0, color.gamma_multiply(0.12));
            painter.rect_stroke(
                selected,
                0.0,
                egui::Stroke::new(2.0, color),
                egui::StrokeKind::Inside,
            );
        }
        painter.text(
            rect.left_top() + egui::vec2(14.0, 14.0),
            egui::Align2::LEFT_TOP,
            format!(
                "WORLD VIEW  /  {:.5}, {:.5}  /  zoom {:.2}",
                state.camera.center.latitude, state.camera.center.longitude, state.camera.zoom
            ),
            egui::FontId::monospace(13.0),
            egui::Color32::WHITE,
        );
        painter.text(
            rect.left_top() + egui::vec2(14.0, 37.0),
            egui::Align2::LEFT_TOP,
            "Visual map only. Simulation uses a separately verified snapshot.",
            egui::FontId::proportional(12.0),
            egui::Color32::WHITE,
        );
        painter.text(
            rect.left_bottom() + egui::vec2(14.0, -14.0),
            egui::Align2::LEFT_BOTTOM,
            "OpenFreeMap / OpenMapTiles / © OpenStreetMap contributors (ODbL)",
            egui::FontId::proportional(12.0),
            egui::Color32::WHITE,
        );
        if !state.basemap_consent {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Basemap disabled. Enable live maps or open a cached region.",
                egui::FontId::proportional(16.0),
                egui::Color32::LIGHT_GRAY,
            );
        }
    });
    if install { state.prepared.take() } else { None }
}
