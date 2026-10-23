//! Web Bluetooth GATT transport adapter for ZMK Studio keyboards in browser environments.

#[cfg(target_arch = "wasm32")]
use crate::platform::web_serial::ZmkRpcMux;
#[cfg(target_arch = "wasm32")]
use crate::protocols::{DeviceError, DeviceEvent};
#[cfg(target_arch = "wasm32")]
use crate::ui_wake::UiWake;
#[cfg(target_arch = "wasm32")]
use keypeek_core::OverlayConfig;
#[cfg(target_arch = "wasm32")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_arch = "wasm32")]
use std::sync::{mpsc, Arc};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(target_arch = "wasm32")]
use zmk_studio_api::proto::zmk::studio;

/// Official ZMK Studio GATT Service UUID.
pub const ZMK_SERVICE_UUID: &str = "00000000-0196-6107-c967-c5cfb1c2482a";
/// Official ZMK Studio RPC GATT Characteristic UUID.
pub const ZMK_RPC_CHAR_UUID: &str = "00000001-0196-6107-c967-c5cfb1c2482a";

/// Maximum payload per BLE ATT write when negotiated MTU is unknown (ATT default 23 - 3 = 20).
/// ZMK Studio firmware treats incoming GATT writes as a continuous byte stream in its ring buffer.
pub const BLE_MAX_WRITE_CHUNK: usize = 20;

/// Returns whether the current browser environment supports the Web Bluetooth API.
#[cfg(target_arch = "wasm32")]
pub fn is_web_bluetooth_supported() -> bool {
    super::web::has_navigator_property("bluetooth")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn is_web_bluetooth_supported() -> bool {
    false
}

/// Returns the OS name (`"Windows"` or `"macOS"`) if the current browser environment blocks
/// direct Web Bluetooth access to already-paired keyboards and requires the WebHID RPC tunnel.
#[cfg(target_arch = "wasm32")]
pub fn ble_blocked_os_name() -> Option<&'static str> {
    let ua = web_sys::window()?.navigator().user_agent().ok()?;
    if ua.contains("Windows") || ua.contains("Win64") || ua.contains("Win32") {
        Some("Windows")
    } else if ua.contains("Macintosh") || ua.contains("Mac OS X") {
        Some("macOS")
    } else {
        None
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn ble_blocked_os_name() -> Option<&'static str> {
    if cfg!(target_os = "windows") {
        Some("Windows")
    } else if cfg!(target_os = "macos") {
        Some("macOS")
    } else {
        None
    }
}

/// Prompts the user to select and pair a ZMK Studio BLE keyboard via Web Bluetooth.
#[cfg(target_arch = "wasm32")]
pub async fn request_web_ble_device() -> Result<Option<web_sys::BluetoothDevice>, DeviceError> {
    let window =
        web_sys::window().ok_or_else(|| DeviceError::Transport("No window found".into()))?;
    if !is_web_bluetooth_supported() {
        return Err(DeviceError::Unsupported(
            "Web Bluetooth is not supported by your browser. Please use Chrome, Edge, or Opera."
                .into(),
        ));
    }

    let bluetooth_val = js_sys::Reflect::get(&window.navigator(), &JsValue::from_str("bluetooth"))
        .map_err(|e| {
            DeviceError::Transport(format!("Failed to access navigator.bluetooth: {e:?}"))
        })?;
    let bluetooth: web_sys::Bluetooth = bluetooth_val
        .dyn_into()
        .map_err(|e| DeviceError::Transport(format!("Expected Bluetooth object: {e:?}")))?;

    let filter = web_sys::BluetoothLeScanFilterInit::new();
    let services = [js_sys::JsString::from(ZMK_SERVICE_UUID)];
    filter.set_services(&services);
    let filters = [filter];

    let options = web_sys::RequestDeviceOptions::new();
    options.set_filters(&filters);

    let device_val =
        match wasm_bindgen_futures::JsFuture::from(bluetooth.request_device(&options)).await {
            Ok(val) => val,
            Err(err) => {
                if super::web::is_user_cancelled_picker(&err) {
                    return Ok(None);
                }
                return Err(DeviceError::Transport(format!(
                    "Web Bluetooth request error: {err:?}"
                )));
            }
        };

    device_val
        .dyn_into()
        .map(Some)
        .map_err(|e| DeviceError::Transport(format!("Expected BluetoothDevice: {e:?}")))
}

/// Web Bluetooth GATT transport for ZMK Studio RPC communication in WebAssembly.
#[cfg(target_arch = "wasm32")]
pub struct WebBleTransport {
    device: web_sys::BluetoothDevice,
    characteristic: web_sys::BluetoothRemoteGattCharacteristic,
    use_write_without_response: bool,
    alive: Arc<AtomicBool>,
    rpc: Arc<ZmkRpcMux>,
    notify_closure: Option<Closure<dyn FnMut(web_sys::Event)>>,
    disconnect_closure: Option<Closure<dyn FnMut(web_sys::Event)>>,
    _ui_wake: UiWake,
}

#[cfg(target_arch = "wasm32")]
unsafe impl Send for WebBleTransport {}
#[cfg(target_arch = "wasm32")]
unsafe impl Sync for WebBleTransport {}

#[cfg(target_arch = "wasm32")]
impl WebBleTransport {
    /// Connects to the ZMK Studio GATT service and RPC characteristic on the given `BluetoothDevice`,
    /// subscribes to notifications, and sets up frame decoding and disconnect listeners.
    pub async fn connect(
        device: web_sys::BluetoothDevice,
        event_tx: mpsc::Sender<DeviceEvent>,
        ui_wake: UiWake,
    ) -> Result<Self, DeviceError> {
        let gatt = device.gatt().ok_or_else(|| {
            DeviceError::Transport("Bluetooth device does not expose a GATT server".into())
        })?;

        let server: web_sys::BluetoothRemoteGattServer =
            wasm_bindgen_futures::JsFuture::from(gatt.connect())
                .await
                .map_err(|e| {
                    DeviceError::Transport(format!("Failed to connect to BLE GATT server: {e:?}"))
                })?
                .dyn_into()
                .map_err(|e| DeviceError::Transport(format!("Expected GATT server: {e:?}")))?;

        let service: web_sys::BluetoothRemoteGattService = wasm_bindgen_futures::JsFuture::from(
            server.get_primary_service_with_str(ZMK_SERVICE_UUID),
        )
        .await
        .map_err(|e| {
            DeviceError::Transport(format!(
                "ZMK Studio GATT service not found on BLE device: {e:?}"
            ))
        })?
        .dyn_into()
        .map_err(|e| DeviceError::Transport(format!("Expected GATT service: {e:?}")))?;

        let characteristic: web_sys::BluetoothRemoteGattCharacteristic =
            wasm_bindgen_futures::JsFuture::from(
                service.get_characteristic_with_str(ZMK_RPC_CHAR_UUID),
            )
            .await
            .map_err(|e| {
                DeviceError::Transport(format!(
                    "ZMK Studio RPC characteristic not found on BLE device: {e:?}"
                ))
            })?
            .dyn_into()
            .map_err(|e| DeviceError::Transport(format!("Expected GATT characteristic: {e:?}")))?;

        let use_write_without_response = characteristic.properties().write_without_response();
        let alive = Arc::new(AtomicBool::new(true));
        let rpc = Arc::new(ZmkRpcMux::new());

        let char_for_notify = characteristic.clone();
        let rpc_for_notify = Arc::clone(&rpc);
        let event_tx_for_notify = event_tx.clone();
        let wake_for_notify = ui_wake.clone();

        let notify_closure =
            Closure::<dyn FnMut(web_sys::Event)>::new(move |_event: web_sys::Event| {
                if let Some(data_view) = char_for_notify.value() {
                    let chunk = super::web::data_view_to_vec(&data_view);
                    rpc_for_notify.push_bytes(&chunk, &event_tx_for_notify, &wake_for_notify);
                }
            });

        characteristic
            .add_event_listener_with_callback(
                "characteristicvaluechanged",
                notify_closure.as_ref().unchecked_ref(),
            )
            .map_err(|e| {
                DeviceError::Transport(format!(
                    "Failed to attach characteristicvaluechanged listener: {e:?}"
                ))
            })?;

        wasm_bindgen_futures::JsFuture::from(characteristic.start_notifications())
            .await
            .map_err(|e| {
                DeviceError::Transport(format!(
                    "Failed to start GATT notifications on ZMK RPC characteristic: {e:?}"
                ))
            })?;

        let alive_for_disc = Arc::clone(&alive);
        let rpc_for_disc = Arc::clone(&rpc);
        let wake_for_disc = ui_wake.clone();

        let disconnect_closure =
            Closure::<dyn FnMut(web_sys::Event)>::new(move |_event: web_sys::Event| {
                if alive_for_disc.swap(false, Ordering::Relaxed) {
                    rpc_for_disc.clear_pending();
                    let _ = event_tx.send(DeviceEvent::Disconnected(
                        "ZMK Bluetooth device disconnected".into(),
                    ));
                    wake_for_disc.request_repaint();
                }
            });

        let _ = device.add_event_listener_with_callback(
            "gattserverdisconnected",
            disconnect_closure.as_ref().unchecked_ref(),
        );

        Ok(Self {
            device,
            characteristic,
            use_write_without_response,
            alive,
            rpc,
            notify_closure: Some(notify_closure),
            disconnect_closure: Some(disconnect_closure),
            _ui_wake: ui_wake,
        })
    }

    /// Access the underlying `web_sys::BluetoothDevice`.
    pub fn device(&self) -> &web_sys::BluetoothDevice {
        &self.device
    }

    /// Asynchronously writes framed bytes to the ZMK Studio RPC characteristic in ATT-safe chunks.
    async fn send_bytes(&self, data: &[u8]) -> Result<(), DeviceError> {
        for chunk in data.chunks(BLE_MAX_WRITE_CHUNK) {
            if !self.alive.load(Ordering::Relaxed) {
                return Err(DeviceError::Transport("BLE device disconnected".into()));
            }
            let uint8_arr = js_sys::Uint8Array::new_with_length(chunk.len() as u32);
            uint8_arr.copy_from(chunk);

            let promise = if self.use_write_without_response {
                self.characteristic
                    .write_value_without_response_with_u8_array(&uint8_arr)
                    .or_else(|_| self.characteristic.write_value_with_u8_array(&uint8_arr))
            } else {
                self.characteristic
                    .write_value_with_response_with_u8_array(&uint8_arr)
                    .or_else(|_| self.characteristic.write_value_with_u8_array(&uint8_arr))
            }
            .map_err(|e| DeviceError::Transport(format!("BLE GATT write call failed: {e:?}")))?;

            wasm_bindgen_futures::JsFuture::from(promise)
                .await
                .map_err(|e| DeviceError::Transport(format!("BLE GATT write failed: {e:?}")))?;
        }
        Ok(())
    }

    /// Sends a ZMK Studio RPC request over BLE GATT and awaits the corresponding response.
    pub async fn call(
        &self,
        subsystem: studio::request::Subsystem,
    ) -> Result<studio::RequestResponse, DeviceError> {
        if !self.alive.load(Ordering::Relaxed) {
            return Err(DeviceError::Transport("BLE device disconnected".into()));
        }
        self.rpc
            .call(subsystem, 3, 2000, |bytes| async move {
                self.send_bytes(&bytes).await
            })
            .await
    }

    /// Detaches event listeners and disconnects the GATT server.
    pub fn close(&mut self) {
        self.alive.store(false, Ordering::Relaxed);
        self.rpc.clear_pending();

        if let Some(closure) = self.notify_closure.take() {
            let _ = self.characteristic.remove_event_listener_with_callback(
                "characteristicvaluechanged",
                closure.as_ref().unchecked_ref(),
            );
        }
        if let Some(closure) = self.disconnect_closure.take() {
            let _ = self.device.remove_event_listener_with_callback(
                "gattserverdisconnected",
                closure.as_ref().unchecked_ref(),
            );
        }
        if let Some(gatt) = self.device.gatt() {
            if gatt.connected() {
                gatt.disconnect();
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for WebBleTransport {
    fn drop(&mut self) {
        self.close();
    }
}

/// Prompts the user to select a ZMK Studio BLE keyboard via Web Bluetooth,
/// connects the GATT RPC transport, and returns the [`WebConnectOutcome`].
#[cfg(target_arch = "wasm32")]
pub async fn request_and_connect_zmk_ble(
    overlay_config: OverlayConfig,
    ui_wake: UiWake,
) -> Result<Option<super::web_hid::WebConnectOutcome>, DeviceError> {
    let Some(device) = request_web_ble_device().await? else {
        return Ok(None);
    };

    let ble_name = device
        .name()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "ZMK BLE Keyboard".to_string());
    let device_id = device.id();

    let (event_tx, event_rx) = mpsc::channel();
    let transport =
        Arc::new(WebBleTransport::connect(device, event_tx.clone(), ui_wake.clone()).await?);

    let hid_transport = crate::platform::web_hid::WebHidTransport::open_paired_by_name(
        &ble_name,
        event_tx.clone(),
        ui_wake.clone(),
    )
    .await;

    let (vid, pid) = hid_transport
        .as_ref()
        .map(|h| (h.device().vendor_id(), h.device().product_id()))
        .unwrap_or((0x1D50, 0x615E));

    let pending_telemetry =
        hid_transport
            .is_none()
            .then_some(super::web_hid::PendingZmkTelemetry {
                vid: None,
                pid: None,
                event_tx,
            });

    crate::firmware::zmk::web::finish_zmk_web_connection(
        crate::firmware::zmk::web::WebZmkTransport::Ble(transport),
        ble_name,
        vid,
        pid,
        "Web Bluetooth",
        crate::protocols::ZmkTransportConfig::Ble(device_id),
        event_rx,
        hid_transport,
        pending_telemetry,
        overlay_config,
        ui_wake,
    )
    .await
    .map(Some)
}
