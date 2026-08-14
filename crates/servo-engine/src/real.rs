#![cfg(feature = "servo-backend")]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use dpi::PhysicalSize;
use embedder_traits::EventLoopWaker;
use engine_api::contract::{
    BrowserEngine, EngineCapabilities, EngineCommand, EngineDescriptor, EngineError,
    EngineInstanceId, EngineInstanceSpec, InputEvent, LifecycleState, PointerButton,
    ENGINE_API_VERSION,
};
use engine_api::surface::{SurfaceType, Viewport};
use euclid::Scale;
use keyboard_types::{Key, KeyState};
use servo::{
    DeviceIntPoint, DeviceIntRect, DevicePoint, InputEvent as ServoInputEvent, KeyboardEvent,
    LoadStatus, MouseButton, MouseButtonAction, MouseButtonEvent, MouseMoveEvent, RenderingContext,
    Servo, ServoBuilder, SoftwareRenderingContext, WebView, WebViewBuilder, WebViewDelegate,
    WebViewPoint,
};
use servo_default_resources as _;
use sha2::{Digest, Sha256};
use url::Url;

pub const SERVO_SURFACE_STRATEGY: &str = "software-rendering-context";
const MAX_TEXT_BYTES: usize = 4096;

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Clone, Default)]
struct ServoWaker {
    awakened: Arc<AtomicBool>,
}

impl EventLoopWaker for ServoWaker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }

    fn wake(&self) {
        self.awakened.store(true, Ordering::Release);
    }
}

struct ServoDelegate {
    context: Rc<SoftwareRenderingContext>,
    frame_count: Arc<AtomicU64>,
    current_url: Arc<Mutex<Option<String>>>,
    title: Arc<Mutex<Option<String>>>,
    frame_digest: Arc<Mutex<Option<String>>>,
    load_complete: Arc<AtomicBool>,
    screenshot_ready: Arc<AtomicBool>,
}

impl WebViewDelegate for ServoDelegate {
    fn notify_url_changed(&self, _webview: WebView, url: Url) {
        *self.current_url.lock().expect("URL mutex poisoned") = Some(url.to_string());
    }

    fn notify_page_title_changed(&self, _webview: WebView, title: Option<String>) {
        *self.title.lock().expect("title mutex poisoned") = title;
    }

    fn notify_load_status_changed(&self, webview: WebView, status: LoadStatus) {
        if status == LoadStatus::Complete {
            self.load_complete.store(true, Ordering::Release);
            let screenshot_ready = self.screenshot_ready.clone();
            webview.take_screenshot(None, move |result| {
                if result.is_ok() {
                    screenshot_ready.store(true, Ordering::Release);
                }
            });
        }
    }

    fn notify_new_frame_ready(&self, webview: WebView) {
        self.context.prepare_for_rendering();
        webview.paint();
        let size = self.context.size();
        let rect = DeviceIntRect::new(
            DeviceIntPoint::new(0, 0),
            DeviceIntPoint::new(size.width as i32, size.height as i32),
        );
        if let Some(image) = self.context.read_to_image(rect) {
            let digest = Sha256::digest(image.as_raw());
            *self
                .frame_digest
                .lock()
                .expect("frame digest mutex poisoned") = Some(hex_digest(digest));
        }
        self.context.present();
        self.frame_count.fetch_add(1, Ordering::AcqRel);
    }
}

struct ServoInstance {
    servo: Servo,
    webview: Option<WebView>,
    context: Rc<SoftwareRenderingContext>,
    waker: ServoWaker,
    frame_count: Arc<AtomicU64>,
    current_url: Arc<Mutex<Option<String>>>,
    title: Arc<Mutex<Option<String>>>,
    frame_digest: Arc<Mutex<Option<String>>>,
    load_complete: Arc<AtomicBool>,
    screenshot_ready: Arc<AtomicBool>,
    viewport: Viewport,
    state: LifecycleState,
    thread_id: thread::ThreadId,
}

impl ServoInstance {
    fn create(spec: &EngineInstanceSpec) -> Result<Self, EngineError> {
        if spec.surface.surface_type != SurfaceType::Software {
            return Err(EngineError::NotSupported {
                operation: "only software surface is available in the first real adapter".into(),
            });
        }
        if !spec.surface.viewport.is_valid() {
            return Err(EngineError::InvalidPayload {
                reason: "viewport must be at least 1x1".into(),
            });
        }

        let viewport = spec.surface.viewport;
        let context = Rc::new(
            SoftwareRenderingContext::new(PhysicalSize::new(viewport.width, viewport.height))
                .map_err(|error| EngineError::InvalidPayload {
                    reason: format!("software rendering context: {error:?}"),
                })?,
        );
        context
            .make_current()
            .map_err(|error| EngineError::InvalidPayload {
                reason: format!("make software context current: {error:?}"),
            })?;

        let waker = ServoWaker::default();
        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(waker.clone()))
            .build();
        servo.setup_logging();

        let frame_count = Arc::new(AtomicU64::new(0));
        let current_url = Arc::new(Mutex::new(None));
        let title = Arc::new(Mutex::new(None));
        let frame_digest = Arc::new(Mutex::new(None));
        let load_complete = Arc::new(AtomicBool::new(false));
        let screenshot_ready = Arc::new(AtomicBool::new(false));
        let delegate = Rc::new(ServoDelegate {
            context: context.clone(),
            frame_count: frame_count.clone(),
            current_url: current_url.clone(),
            title: title.clone(),
            frame_digest: frame_digest.clone(),
            load_complete: load_complete.clone(),
            screenshot_ready: screenshot_ready.clone(),
        });
        let builder = WebViewBuilder::new(&servo, context.clone())
            .hidpi_scale_factor(Scale::new(1.0))
            .delegate(delegate);
        let webview = match spec.initial_url.as_deref() {
            Some(raw_url) => {
                let url = Url::parse(raw_url).map_err(|error| EngineError::InvalidPayload {
                    reason: format!("initial URL: {error}"),
                })?;
                builder.url(url).build()
            }
            None => builder.build(),
        };
        webview.show();

        let mut instance = Self {
            servo,
            webview: Some(webview),
            context,
            waker,
            frame_count,
            current_url,
            title,
            frame_digest,
            load_complete,
            screenshot_ready,
            viewport,
            state: LifecycleState::Starting,
            thread_id: thread::current().id(),
        };
        instance.pump_once();
        instance.state = if spec.initial_url.is_some() {
            LifecycleState::Navigating
        } else {
            LifecycleState::Ready
        };
        Ok(instance)
    }

    fn pump_once(&self) {
        self.waker.awakened.store(false, Ordering::Release);
        self.servo.spin_event_loop();
    }

    fn ensure_current_thread(&self) -> Result<(), EngineError> {
        if self.thread_id != thread::current().id() {
            return Err(EngineError::InvalidPayload {
                reason: "Servo instance accessed from a different thread".into(),
            });
        }
        Ok(())
    }

    fn validate_point(&self, x: i32, y: i32) -> Result<WebViewPoint, EngineError> {
        if x < 0 || y < 0 || x >= self.viewport.width as i32 || y >= self.viewport.height as i32 {
            return Err(EngineError::InvalidPayload {
                reason: "pointer coordinate is outside the viewport".into(),
            });
        }
        Ok(WebViewPoint::Device(DevicePoint::new(x as f32, y as f32)))
    }

    fn send_input(&self, input: InputEvent) -> Result<(), EngineError> {
        let webview = self.webview.as_ref().ok_or(EngineError::InvalidPayload {
            reason: "webview is closed".into(),
        })?;
        match input {
            InputEvent::PointerMove { x, y } => {
                let point = self.validate_point(x, y)?;
                webview.notify_input_event(ServoInputEvent::MouseMove(MouseMoveEvent::new(point)));
            }
            InputEvent::PointerDown { button, x, y } => {
                let point = self.validate_point(x, y)?;
                webview.focus();
                self.pump_once();
                webview.notify_input_event(ServoInputEvent::MouseButton(MouseButtonEvent::new(
                    MouseButtonAction::Down,
                    map_button(button),
                    point,
                )));
            }
            InputEvent::PointerUp { button, x, y } => {
                let point = self.validate_point(x, y)?;
                webview.notify_input_event(ServoInputEvent::MouseButton(MouseButtonEvent::new(
                    MouseButtonAction::Up,
                    map_button(button),
                    point,
                )));
            }
            InputEvent::Text { text } => {
                if text.is_empty() || text.len() > MAX_TEXT_BYTES || text.contains('\0') {
                    return Err(EngineError::InvalidPayload {
                        reason: "text input must be non-empty, bounded and NUL-free".into(),
                    });
                }
                for character in text.chars() {
                    let key = Key::Character(character.to_string());
                    webview.notify_input_event(ServoInputEvent::Keyboard(
                        KeyboardEvent::from_state_and_key(KeyState::Down, key.clone()),
                    ));
                    webview.notify_input_event(ServoInputEvent::Keyboard(
                        KeyboardEvent::from_state_and_key(KeyState::Up, key),
                    ));
                }
            }
        }
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), EngineError> {
        if width == 0 || height == 0 {
            return Err(EngineError::InvalidPayload {
                reason: "viewport must be at least 1x1".into(),
            });
        }
        let size = PhysicalSize::new(width, height);
        self.context.resize(size);
        self.webview
            .as_ref()
            .ok_or(EngineError::InvalidPayload {
                reason: "webview is closed".into(),
            })?
            .resize(size);
        self.viewport = Viewport::new(width, height);
        Ok(())
    }

    fn shutdown(mut self) -> Result<(), EngineError> {
        self.webview.take();
        drop(self);
        Ok(())
    }

    fn evidence(&self) -> ServoEvidence {
        ServoEvidence {
            servo_revision: super::SERVO_PINNED_SHA,
            surface_strategy: SERVO_SURFACE_STRATEGY,
            viewport: self.viewport,
            frame_count: self.frame_count.load(Ordering::Acquire),
            current_url: self.current_url.lock().expect("URL mutex poisoned").clone(),
            title: self.title.lock().expect("title mutex poisoned").clone(),
            frame_digest: self
                .frame_digest
                .lock()
                .expect("frame digest mutex poisoned")
                .clone(),
            load_complete: self.load_complete.load(Ordering::Acquire),
            screenshot_ready: self.screenshot_ready.load(Ordering::Acquire),
            thread_id: format!("{:?}", self.thread_id),
            on_current_thread: self.thread_id == thread::current().id(),
        }
    }
}

fn map_button(button: PointerButton) -> MouseButton {
    match button {
        PointerButton::Left => MouseButton::Left,
        PointerButton::Middle => MouseButton::Middle,
        PointerButton::Right => MouseButton::Right,
    }
}

pub struct RealServoAdapter {
    instances: RefCell<BTreeMap<String, ServoInstance>>,
}

impl Default for RealServoAdapter {
    fn default() -> Self {
        Self {
            instances: RefCell::new(BTreeMap::new()),
        }
    }
}

impl RealServoAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pump(&self) -> Result<(), EngineError> {
        for instance in self.instances.borrow_mut().values_mut() {
            instance.ensure_current_thread()?;
            instance.pump_once();
        }
        Ok(())
    }

    pub fn evidence(&self, instance_id: &EngineInstanceId) -> Result<ServoEvidence, EngineError> {
        self.instances
            .borrow()
            .get(&instance_id.0)
            .map(ServoInstance::evidence)
            .ok_or_else(|| EngineError::InvalidPayload {
                reason: "instance not found".into(),
            })
    }
}

impl BrowserEngine for RealServoAdapter {
    fn descriptor(&self) -> EngineDescriptor {
        EngineDescriptor {
            name: "servo".into(),
            api_revision: ENGINE_API_VERSION,
        }
    }

    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            can_navigate: true,
            can_reload: true,
            can_go_back: true,
            can_go_forward: true,
            can_resize: true,
            can_receive_input: true,
        }
    }

    fn create(&self, spec: EngineInstanceSpec) -> Result<(), EngineError> {
        if spec.api_version != ENGINE_API_VERSION {
            return Err(EngineError::UnknownVersion {
                version: spec.api_version,
            });
        }
        let id = spec.instance_id.0.clone();
        let mut instances = self.instances.borrow_mut();
        if instances.contains_key(&id) {
            return Err(EngineError::InvalidPayload {
                reason: "instance already exists".into(),
            });
        }
        instances.insert(id, ServoInstance::create(&spec)?);
        Ok(())
    }

    fn destroy(&self, instance_id: &EngineInstanceId) -> Result<(), EngineError> {
        let instance = self
            .instances
            .borrow_mut()
            .remove(&instance_id.0)
            .ok_or_else(|| EngineError::InvalidPayload {
                reason: "instance not found".into(),
            })?;
        instance.shutdown()
    }

    fn send_command(
        &self,
        instance_id: &EngineInstanceId,
        command: EngineCommand,
    ) -> Result<(), EngineError> {
        if command == EngineCommand::Shutdown {
            return self.destroy(instance_id);
        }
        let mut instances = self.instances.borrow_mut();
        let instance =
            instances
                .get_mut(&instance_id.0)
                .ok_or_else(|| EngineError::InvalidPayload {
                    reason: "instance not found".into(),
                })?;
        instance.ensure_current_thread()?;
        if !instance.state.accepts_commands() {
            return Err(EngineError::NotSupported {
                operation: format!("command in state {:?}", instance.state),
            });
        }
        self.capabilities().check(&command)?;

        match command {
            EngineCommand::Navigate { url } => {
                let parsed = Url::parse(&url).map_err(|error| EngineError::InvalidPayload {
                    reason: format!("navigation URL: {error}"),
                })?;
                instance
                    .webview
                    .as_ref()
                    .ok_or_else(|| EngineError::InvalidPayload {
                        reason: "webview is closed".into(),
                    })?
                    .load(parsed);
                instance.state = LifecycleState::Navigating;
            }
            EngineCommand::Reload => instance.webview.as_ref().unwrap().reload(),
            EngineCommand::GoBack => {
                instance.webview.as_ref().unwrap().go_back(1);
            }
            EngineCommand::GoForward => {
                instance.webview.as_ref().unwrap().go_forward(1);
            }
            EngineCommand::Stop => {
                instance.webview.as_ref().unwrap().set_throttled(true);
                instance.state = LifecycleState::Ready;
            }
            EngineCommand::SetViewport { width, height } => instance.resize(width, height)?,
            EngineCommand::Input { event } => instance.send_input(event)?,
            EngineCommand::Shutdown => unreachable!(),
        }
        instance.pump_once();
        Ok(())
    }

    fn state(&self, instance_id: &EngineInstanceId) -> Result<LifecycleState, EngineError> {
        self.instances
            .borrow()
            .get(&instance_id.0)
            .map(|instance| instance.state)
            .ok_or_else(|| EngineError::InvalidPayload {
                reason: "instance not found".into(),
            })
    }
}

#[derive(Debug, Clone)]
pub struct ServoEvidence {
    pub servo_revision: &'static str,
    pub surface_strategy: &'static str,
    pub viewport: Viewport,
    pub frame_count: u64,
    pub current_url: Option<String>,
    pub title: Option<String>,
    pub frame_digest: Option<String>,
    pub load_complete: bool,
    pub screenshot_ready: bool,
    pub thread_id: String,
    pub on_current_thread: bool,
}
