//! MapLibre Native Vulkan worker with bounded, latest-value mailboxes.
//! Uses RGBA readback/upload for the first integration, not zero-copy interop.
#![deny(unsafe_code)]
mod vulkan;
use maplibre_native_ffi as ml;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub latitude: f64,
    pub longitude: f64,
    pub zoom: f64,
    pub width: u32,
    pub height: u32,
}
impl View {
    pub fn valid(self) -> bool {
        self.latitude.is_finite()
            && self.latitude.abs() <= 85.05113
            && self.longitude.is_finite()
            && self.longitude.abs() <= 180.0
            && self.zoom.is_finite()
            && (0.0..=20.0).contains(&self.zoom)
            && (32..=2048).contains(&self.width)
            && (32..=1536).contains(&self.height)
    }
}
#[derive(Debug)]
pub struct Frame {
    pub view: View,
    pub rgba: Vec<u8>,
    pub complete: bool,
}
#[derive(Clone)]
pub struct Options {
    pub cache_path: PathBuf,
    pub style_url: String,
    pub software: bool,
    pub fixture: bool,
}
pub struct BasemapWorker {
    request: Arc<Mutex<Option<View>>>,
    output: Arc<Mutex<Option<Result<Frame, String>>>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl BasemapWorker {
    pub fn new(options: Options) -> Self {
        let request = Arc::new(Mutex::new(None));
        let output = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (r, o, s) = (request.clone(), output.clone(), stop.clone());
        let thread = thread::spawn(move || {
            let result = run(options, &r, &o, &s);
            if let Err(e) = result
                && let Ok(mut output) = o.lock()
            {
                *output = Some(Err(e));
            }
        });
        Self {
            request,
            output,
            stop,
            thread: Some(thread),
        }
    }
    pub fn request(&self, view: View) {
        if view.valid()
            && let Ok(mut slot) = self.request.lock()
        {
            *slot = Some(view);
        }
    }
    pub fn poll(&self) -> Option<Result<Frame, String>> {
        self.output.lock().ok()?.take()
    }
}
impl Drop for BasemapWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // The worker owns all native handles and destroys them on its owner thread.
        // Never block the UI waiting for a driver or an HTTP cancellation.
        let _ = self.thread.take();
    }
}
struct NativeMap {
    // Rust drops fields in declaration order. Native resources precede their Vulkan owner.
    session: ml::RenderSessionHandle,
    map: ml::MapHandle,
    runtime: ml::RuntimeHandle,
    _gpu: vulkan::Vulkan,
    view: View,
}
impl NativeMap {
    fn new(options: &Options, view: View) -> Result<Self, String> {
        if let Some(parent) = options.cache_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let gpu = vulkan::Vulkan::new(options.software)?;
        let mut runtime_options = ml::RuntimeOptions::default();
        runtime_options.cache_path = Some(options.cache_path.to_string_lossy().into_owned());
        let runtime =
            ml::RuntimeHandle::with_options(&runtime_options).map_err(|e| e.to_string())?;
        runtime
            .set_http_header_transform(|_| {
                vec![ml::HttpHeader::new(
                    "User-Agent",
                    "OpenStreetBattle/0.2 (+https://github.com/Vaspyyy/OpenStreetBattle)",
                )]
            })
            .map_err(|e| e.to_string())?;
        let map = ml::MapHandle::with_options(
            &runtime,
            &ml::MapOptions::new(view.width, view.height, 1.0),
        )
        .map_err(|e| e.to_string())?;
        if options.fixture {
            map.set_style_json(FIXTURE_STYLE.as_bytes())
                .map_err(|e| e.to_string())?;
        } else {
            map.set_style_url(&options.style_url)
                .map_err(|e| e.to_string())?;
        }
        let descriptor = ml::VulkanOwnedTextureDescriptor::new(
            ml::RenderTargetExtent::new(view.width, view.height, 1.0),
            gpu.descriptor(),
        );
        let session = map
            .attach_ref()
            .map_err(|e| e.to_string())?
            .attach_vulkan_owned_texture(&descriptor)
            .map_err(|e| e.to_string())?;
        let mut result = Self {
            session,
            map,
            runtime,
            _gpu: gpu,
            view,
        };
        result.set_view(view)?;
        Ok(result)
    }
    fn set_view(&mut self, view: View) -> Result<(), String> {
        if self.view.width != view.width || self.view.height != view.height {
            self.session
                .resize(view.width, view.height, 1.0)
                .map_err(|e| e.to_string())?;
        }
        let mut camera = ml::CameraOptions::default();
        camera.center = Some(ml::LatLng::new(view.latitude, view.longitude));
        camera.zoom = Some(view.zoom);
        camera.bearing = Some(0.0);
        camera.pitch = Some(0.0);
        self.map.jump_to(&camera).map_err(|e| e.to_string())?;
        self.view = view;
        Ok(())
    }
    fn frame(&mut self) -> Result<Option<Frame>, String> {
        self.runtime
            .pump(Some(Duration::ZERO), Some(Duration::from_millis(3)))
            .map_err(|e| e.to_string())?;
        {
            let events = self.runtime.drain_events(256).map_err(|e| e.to_string())?;
            for e in events.iter() {
                if e.event_type() == ml::RuntimeEventType::MapLoadingFailed {
                    return Err(e
                        .message()
                        .ok()
                        .flatten()
                        .unwrap_or("map style loading failed")
                        .to_owned());
                }
            }
        }
        let render = self.session.render_update().map_err(|e| e.to_string())?;
        if render.result != ml::RenderResult::Rendered {
            return Ok(None);
        }
        let info = self
            .session
            .texture_image_info()
            .map_err(|e| e.to_string())?;
        if info.width != self.view.width || info.height != self.view.height {
            return Ok(None);
        }
        if info.byte_length > 16 * 1024 * 1024 || info.stride < info.width * 4 {
            return Err("invalid/oversized native texture".into());
        }
        let mut bytes = vec![0; info.byte_length];
        self.session
            .read_premultiplied_rgba8_into(&mut bytes)
            .map_err(|e| e.to_string())?;
        let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
        for row in 0..info.height as usize {
            let start = row * info.stride as usize;
            let end = start + info.width as usize * 4;
            rgba.extend_from_slice(bytes.get(start..end).ok_or("invalid texture row stride")?);
        }
        let complete = self.map.is_fully_loaded().map_err(|e| e.to_string())?;
        Ok(Some(Frame {
            view: self.view,
            rgba,
            complete,
        }))
    }
}
type RequestSlot = Arc<Mutex<Option<View>>>;
type OutputSlot = Arc<Mutex<Option<Result<Frame, String>>>>;
fn run(
    options: Options,
    request: &RequestSlot,
    output: &OutputSlot,
    stop: &Arc<AtomicBool>,
) -> Result<(), String> {
    let mut native: Option<NativeMap> = None;
    let mut dirty = true;
    let mut last = Instant::now() - Duration::from_secs(1);
    while !stop.load(Ordering::Relaxed) {
        let view = request
            .lock()
            .map_err(|_| "map request lock poisoned")?
            .take();
        if let Some(view) = view {
            if let Some(map) = native.as_mut() {
                if view != map.view {
                    map.set_view(view)?;
                    dirty = true;
                }
            } else {
                native = Some(NativeMap::new(&options, view)?);
            }
        }
        if let Some(map) = native.as_mut()
            && dirty
            && last.elapsed() >= Duration::from_millis(100)
        {
            if let Some(frame) = map.frame()? {
                dirty = !frame.complete;
                *output.lock().map_err(|_| "map output lock poisoned")? = Some(Ok(frame));
            }
            last = Instant::now();
        }
        thread::sleep(Duration::from_millis(15));
    }
    Ok(())
}
/// Synthetic GeoJSON fixture exercises actual Vulkan drawing without external services or fonts.
pub const FIXTURE_STYLE: &str = r##"{"version":8,"sources":{"fixture":{"type":"geojson","data":{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[9.99,53.99],[10.02,53.99],[10.02,54.02],[9.99,54.02],[9.99,53.99]]]}}]}}},"layers":[{"id":"background","type":"background","paint":{"background-color":"#26383b"}},{"id":"region","type":"fill","source":"fixture","paint":{"fill-color":"#769182"}},{"id":"edge","type":"line","source":"fixture","paint":{"line-color":"#d4d7b5","line-width":4}}]}"##;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_views() {
        assert!(
            !View {
                latitude: f64::NAN,
                longitude: 0.0,
                zoom: 2.0,
                width: 100,
                height: 100
            }
            .valid()
        );
    }
    #[test]
    #[ignore = "requires a working software Vulkan driver"]
    fn native_vulkan_fixture_really_renders() {
        let worker = BasemapWorker::new(Options {
            cache_path: std::env::temp_dir()
                .join(format!("osb-map-test-{}/cache.db", std::process::id())),
            style_url: String::new(),
            software: true,
            fixture: true,
        });
        worker.request(View {
            latitude: 54.005,
            longitude: 10.005,
            zoom: 12.0,
            width: 320,
            height: 240,
        });
        let start = Instant::now();
        loop {
            if let Some(result) = worker.poll() {
                let f = result.unwrap();
                assert_eq!(f.rgba.len(), 320 * 240 * 4);
                if f.complete {
                    assert!(f.rgba.chunks_exact(4).any(|p| p != &f.rgba[0..4]));
                    break;
                }
            }
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "native renderer did not complete"
            );
            thread::sleep(Duration::from_millis(30));
        }
    }
}
