use std::sync::Arc;

use crate::application::keyboard::Keyboard;
use crate::device_discovery::DiscoveredDevice;
use crate::protocols::{DeviceError, KeyboardProtocol};
use crate::ui_wake::UiWake;
use keypeek_core::keymap_editor::EditorProfile;
pub use keypeek_core::OverlayConfig;
use wasm_bindgen::JsValue;

/// Async timer utility that resolves after `ms` milliseconds using browser `setTimeout`.
pub async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Checks whether `window.navigator` exposes the given property name (e.g. `"hid"`, `"serial"`, `"bluetooth"`).
pub fn has_navigator_property(prop: &str) -> bool {
    web_sys::window()
        .and_then(|w| js_sys::Reflect::has(&w.navigator(), &JsValue::from_str(prop)).ok())
        .unwrap_or(false)
}

/// Returns `true` if a `requestDevice` / `requestPort` rejection corresponds to the user dismissing the browser picker.
pub fn is_user_cancelled_picker(err: &JsValue) -> bool {
    let err_str = format!("{err:?}");
    err_str.contains("NotFoundError")
        || err_str.contains("AbortError")
        || err_str.contains("cancel")
        || err_str.contains("User cancelled")
        || err_str.contains("user did not select")
}

/// Copies the bytes of a `js_sys::DataView` into a `Vec<u8>`.
pub fn data_view_to_vec(view: &js_sys::DataView) -> Vec<u8> {
    let len = view.byte_length();
    let mut buf = vec![0u8; len];
    for (i, b) in buf.iter_mut().enumerate() {
        *b = view.get_uint8(i);
    }
    buf
}

pub struct ConnectedWebDevice {
    pub device: DiscoveredDevice,
    pub keyboard: Arc<Keyboard>,
    pub profile: Arc<dyn EditorProfile>,
}

impl ConnectedWebDevice {
    /// Constructs a connected web device, selecting the correct presenter and editor profile
    /// via the firmware bundle registered for the device's connection spec.
    pub fn new(
        device: DiscoveredDevice,
        protocol: impl KeyboardProtocol + 'static,
        overlay_config: OverlayConfig,
        ui_wake: UiWake,
    ) -> Result<Self, DeviceError> {
        let bundle = crate::firmware::bundle_for_spec(&device.spec);
        let selected_layout = protocol
            .get_layout_definition()
            .get_layout_names()
            .first()
            .cloned()
            .unwrap_or_else(|| "default".to_string());

        let keyboard = Keyboard::new(
            Box::new(protocol),
            selected_layout,
            overlay_config,
            ui_wake,
            bundle.create_presenter(),
        )
        .map_err(DeviceError::Protocol)?;

        Ok(Self {
            device,
            keyboard: Arc::new(keyboard),
            profile: bundle.create_profile(),
        })
    }
}
