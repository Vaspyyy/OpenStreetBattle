#[cfg(not(target_os = "linux"))]
compile_error!(
    "OpenStreetBattle's native client currently supports Linux/Wayland only. The core is platform-independent."
);

mod ui;
use bevy::{
    prelude::*,
    render::{
        RenderPlugin,
        renderer::RenderAdapterInfo,
        settings::{Backends, WgpuSettings},
        view::screenshot::{Screenshot, save_to_disk},
    },
    window::PresentMode,
};
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass, EguiStartupSet};
use clap::Parser;
use osb_map::MapCamera;
use osb_sim::{DT, Scenario, Simulation, Soldier, SoldierId};
use osb_world::Point;
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Parser)]
#[command(version, about = "OpenStreetBattle native Wayland/Vulkan observer")]
struct Args {
    scenario: Option<PathBuf>,
    #[arg(long, conflicts_with = "scenario")]
    map: Option<PathBuf>,
    #[arg(long,conflicts_with_all=["scenario","map","seed"])]
    load: Option<PathBuf>,
    #[arg(long)]
    seed: Option<u64>,
    #[arg(long)]
    save_path: Option<PathBuf>,
    /// CI/development smoke test: exit after this many rendered frames.
    #[arg(long)]
    smoke_frames: Option<u32>,
    #[arg(long)]
    screenshot: Option<PathBuf>,
    /// Explicit software Vulkan adapter for CI. Never selects OpenGL.
    #[arg(long)]
    software_renderer: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditMode {
    Inspect,
    BlueSpawn,
    RedSpawn,
    BlueObjective,
    RedObjective,
}
struct ClientState {
    sim: Simulation,
    view: Vec<Soldier>,
    previous: BTreeMap<SoldierId, Point>,
    camera: MapCamera,
    selected: Option<SoldierId>,
    follow: bool,
    paused: bool,
    speed: f64,
    max_speed: bool,
    accumulator: f64,
    draft: Scenario,
    seed: u64,
    mode: EditMode,
    show_routes: bool,
    show_groups: bool,
    show_contacts: bool,
    file_path: String,
    save_path: PathBuf,
    status: String,
    last_tick_ms: f64,
    frame: u64,
}
impl ClientState {
    fn refresh(&mut self) {
        self.view = self.sim.soldiers();
    }
    fn step(&mut self) {
        self.previous = self.view.iter().map(|s| (s.id, s.position)).collect();
        let start = Instant::now();
        match self.sim.step() {
            Ok(()) => self.refresh(),
            Err(e) => {
                self.paused = true;
                self.status = e.to_string();
            }
        }
        self.last_tick_ms = start.elapsed().as_secs_f64() * 1000.0;
    }
    fn rebuild(&mut self) {
        match Simulation::new(self.draft.clone(), self.seed) {
            Ok(sim) => {
                self.sim = sim;
                self.paused = true;
                self.accumulator = 0.0;
                self.previous.clear();
                self.refresh();
                self.status = "Scenario rebuilt. Ready to observe.".into();
            }
            Err(e) => {
                self.draft = self.sim.scenario().clone();
                self.seed = self.sim.snapshot().seed;
                self.status = format!("Edit rejected; previous scenario retained: {e}");
            }
        }
    }
    fn save(&mut self) {
        let result = (|| -> Result<(), Box<dyn Error>> {
            if let Some(parent) = self.save_path.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)?;
            }
            osb_campaign::save(&self.save_path, &self.sim.snapshot())?;
            Ok(())
        })();
        self.status = match result {
            Ok(()) => format!("Checkpoint saved: {}", self.save_path.display()),
            Err(e) => format!("Save failed: {e}"),
        };
    }
    fn load_checkpoint(&mut self) {
        let result = osb_campaign::load(&self.save_path)
            .map_err(|e| e.to_string())
            .and_then(|s| Simulation::restore(s).map_err(|e| e.to_string()));
        match result {
            Ok(sim) => {
                self.sim = sim;
                self.seed = self.sim.snapshot().seed;
                self.draft = self.sim.scenario().clone();
                self.camera = MapCamera::new(self.draft.map.bounds.center());
                self.refresh();
                self.selected = self.view.first().map(|s| s.id);
                self.follow = false;
                self.mode = EditMode::Inspect;
                self.previous.clear();
                self.accumulator = 0.0;
                self.paused = true;
                self.status = "Checkpoint restored. Simulation paused.".into();
            }
            Err(e) => self.status = format!("Load failed: {e}"),
        }
    }
    fn display_position(&self, s: &Soldier) -> Point {
        if self.paused || self.max_speed {
            return s.position;
        }
        let old = self.previous.get(&s.id).copied().unwrap_or(s.position);
        let t = (self.accumulator / DT).clamp(0.0, 1.0);
        Point::new(
            old.x + (s.position.x - old.x) * t,
            old.y + (s.position.y - old.y) * t,
        )
    }
    fn import_map(&mut self) {
        let result = read_bounded(Path::new(&self.file_path), 32 * 1024 * 1024)
            .map_err(|e| e.to_string())
            .and_then(|s| osb_world::import_osm_json(&s).map_err(|e| e.to_string()))
            .and_then(|m| Scenario::on_map(m).map_err(|e| e.to_string()));
        match result {
            Ok(scenario) => {
                self.camera.center = scenario.map.bounds.center();
                self.draft = scenario;
                self.rebuild();
            }
            Err(e) => self.status = e,
        }
    }
}
#[derive(Resource)]
struct Smoke {
    frames: Option<u32>,
    screenshot: Option<PathBuf>,
    frame: u32,
    captured: bool,
}
fn read_bounded(path: &Path, max: usize) -> Result<String, Box<dyn Error>> {
    let mut text = String::new();
    fs::File::open(path)?
        .take((max + 1) as u64)
        .read_to_string(&mut text)?;
    if text.len() > max {
        return Err("input exceeds size limit".into());
    }
    Ok(text)
}
fn default_save_path() -> PathBuf {
    if let Some(p) = std::env::var_os("XDG_STATE_HOME") {
        PathBuf::from(p).join("openstreetbattle/checkpoint.osb")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".local/state/openstreetbattle/checkpoint.osb")
    } else {
        PathBuf::from("saves/checkpoint.osb")
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Err("Native Wayland session required: WAYLAND_DISPLAY is unset. XWayland/X11 is intentionally not enabled.".into());
    }
    if args.smoke_frames.is_some_and(|n| n < 30) {
        return Err("--smoke-frames must be at least 30".into());
    }
    let sim = if let Some(path) = args.load.as_ref() {
        Simulation::restore(osb_campaign::load(path)?)?
    } else {
        let scenario = if let Some(path) = args.scenario.as_ref() {
            Scenario::parse(&read_bounded(path, 64 * 1024 * 1024)?)?
        } else if let Some(path) = args.map.as_ref() {
            Scenario::on_map(osb_world::import_osm_json(&read_bounded(
                path,
                32 * 1024 * 1024,
            )?)?)?
        } else {
            Scenario::demo()
        };
        Simulation::new(scenario, args.seed.unwrap_or(42))?
    };
    let seed = sim.snapshot().seed;
    let camera = MapCamera::new(sim.scenario().map.bounds.center());
    let draft = sim.scenario().clone();
    let view = sim.soldiers();
    let selected = view.first().map(|s| s.id);
    let state = ClientState {
        sim,
        view,
        previous: BTreeMap::new(),
        camera,
        selected,
        follow: false,
        paused: true,
        speed: 1.0,
        max_speed: false,
        accumulator: 0.0,
        draft,
        seed,
        mode: EditMode::Inspect,
        show_routes: true,
        show_groups: false,
        show_contacts: true,
        file_path: String::new(),
        save_path: args
            .save_path
            .or(args.load)
            .unwrap_or_else(default_save_path),
        status:
            "Offline geometry backend. Press Play to observe. No external services or API keys."
                .into(),
        last_tick_ms: 0.0,
        frame: 0,
    };
    App::new()
        .insert_non_send(state)
        .insert_resource(Smoke {
            frames: args.smoke_frames,
            screenshot: args.screenshot,
            frame: 0,
            captured: false,
        })
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "OpenStreetBattle | Foundation".into(),
                        resolution: (1440, 900).into(),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: WgpuSettings {
                        backends: Some(Backends::VULKAN),
                        force_fallback_adapter: args.software_renderer,
                        ..default()
                    }
                    .into(),
                    ..default()
                }),
        )
        .add_plugins(EguiPlugin::default())
        .add_systems(PreStartup, setup.before(EguiStartupSet::InitContexts))
        .add_systems(Update, (tick, smoke).chain())
        .add_systems(EguiPrimaryContextPass, ui::draw)
        .run();
    Ok(())
}
fn setup(mut commands: Commands, adapter: Res<RenderAdapterInfo>) {
    commands.spawn(Camera2d);
    info!(
        "OSB_NATIVE_WAYLAND adapter={} backend={:?}",
        adapter.name, adapter.backend
    );
}
fn tick(time: Res<Time<Real>>, mut state: NonSendMut<ClientState>) {
    state.frame += 1;
    if state.paused {
        return;
    }
    state.accumulator += time.delta_secs_f64().min(0.25) * state.speed;
    let start = Instant::now();
    let mut steps = 0;
    while (state.max_speed || state.accumulator >= DT)
        && start.elapsed().as_secs_f64() < 0.006
        && steps < 256
        && !state.paused
    {
        state.step();
        steps += 1;
        if !state.max_speed {
            state.accumulator -= DT;
        } else {
            state.accumulator = 0.0;
        }
    }
}
fn smoke(
    mut test: ResMut<Smoke>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut state: NonSendMut<ClientState>,
) {
    test.frame += 1;
    if test.frames.is_some() && test.frame == 2 {
        state.paused = false;
        state.speed = 5.0;
    }
    if test.frame == 20
        && !test.captured
        && let Some(path) = test.screenshot.as_ref()
    {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()));
        test.captured = true;
    }
    if test.frames.is_some_and(|n| test.frame >= n) {
        info!(
            "OSB_SMOKE_OK tick={} people={}",
            state.sim.tick(),
            state.view.len()
        );
        exit.write(AppExit::Success);
    }
}
