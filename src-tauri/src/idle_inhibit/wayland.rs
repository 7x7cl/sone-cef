use raw_window_handle::{
    HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::idle_inhibit::zv1::client::{
    zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1,
    zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1,
};

/// Minimal dispatch sink. We only ever send requests (bind, create_inhibitor,
/// destroy); none of these objects emit events we care about.
struct State;

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(_: &mut Self, _: &WlRegistry, _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ZwpIdleInhibitManagerV1, ()> for State {
    fn event(_: &mut Self, _: &ZwpIdleInhibitManagerV1,
        _: <ZwpIdleInhibitManagerV1 as Proxy>::Event, _: &(),
        _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ZwpIdleInhibitorV1, ()> for State {
    fn event(_: &mut Self, _: &ZwpIdleInhibitorV1,
        _: <ZwpIdleInhibitorV1 as Proxy>::Event, _: &(),
        _: &Connection, _: &QueueHandle<Self>) {}
}

/// All fields are Send (wayland-client sys objects are Send+Sync).
pub struct WaylandInhibitor {
    _conn: Connection,
    inhibitor: ZwpIdleInhibitorV1,
}

impl WaylandInhibitor {
    /// Build an inhibitor for the given window. Returns None if not on
    /// Wayland, the surface isn't realized, or the compositor lacks
    /// idle-inhibit.
    fn build(window: &tauri::WebviewWindow) -> Option<Self> {
        let display = window.display_handle().ok()?;
        let handle = window.window_handle().ok()?;
        let (display_ptr, surface_ptr) = match (display.as_raw(), handle.as_raw()) {
            (RawDisplayHandle::Wayland(d), RawWindowHandle::Wayland(w)) => {
                (d.display.as_ptr(), w.surface.as_ptr())
            }
            _ => {
                log::warn!(
                    "wl_display/wl_surface unavailable (not realized or not Wayland)"
                );
                return None;
            }
        };

        let backend = unsafe { Backend::from_foreign_display(display_ptr as *mut _) };
        let conn = Connection::from_backend(backend);

        let (globals, mut queue) = registry_queue_init::<State>(&conn).ok()?;
        let qh = queue.handle();
        let manager: ZwpIdleInhibitManagerV1 = match globals.bind::<ZwpIdleInhibitManagerV1, _, _>(&qh, 1..=1, ()) {
            Ok(m) => m,
            Err(e) => {
                log::warn!("compositor has no zwp_idle_inhibit_manager_v1: {e}");
                return None;
            }
        };

        let id = unsafe {
            ObjectId::from_ptr(WlSurface::interface(), surface_ptr as *mut _).ok()?
        };
        let surface = WlSurface::from_id(&conn, id).ok()?;

        let inhibitor = manager.create_inhibitor(&surface, &qh, ());
        let _ = queue.flush();
        let mut state = State;
        let _ = queue.roundtrip(&mut state);
        log::info!("Wayland idle inhibitor created");

        Some(Self { _conn: conn, inhibitor })
    }

    /// Public entry. The native handles are stable pointers under
    /// winit/CEF — readable from any thread, no main-thread hop needed.
    pub fn start(window: &tauri::WebviewWindow) -> Option<Self> {
        Self::build(window)
    }

    pub fn stop(self) {
        self.inhibitor.destroy();
        log::info!("Wayland idle inhibitor destroyed");
    }
}
