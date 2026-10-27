//! WebHID transport adapter and companion packet demuxer for browser environments.

use crate::platform::web_serial::ZmkRpcMux;
use crate::protocols::{decode_raw_hid_packet, DeviceError, DeviceEvent};
use crate::ui_wake::UiWake;
#[cfg(target_arch = "wasm32")]
use keypeek_core::OverlayConfig;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{mpsc, Arc, Mutex};
use std::task::{Context, Poll, Waker};
#[cfg(target_arch = "wasm32")]
use zmk_studio_api::proto::zmk::studio;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

/// Maximum size of a standard raw HID / VIA report.
pub const RAW_REPORT_SIZE: usize = 32;

/// Header byte identifying a ZMK Studio RPC stream chunk inside a 32-byte Raw HID report.
/// Format: `[0xF2, payload_len (1..=30), ...payload_bytes]`.
pub const ZMK_RPC_HID_TAG: u8 = 0xF2;

/// Maximum number of ZMK Studio RPC stream bytes carried in a single 32-byte HID report.
pub const ZMK_RPC_HID_MAX_PAYLOAD: usize = RAW_REPORT_SIZE - 2;

/// Encodes a SLIP-framed ZMK Studio RPC byte slice into 32-byte Raw HID reports tagged with [`ZMK_RPC_HID_TAG`].
pub fn encode_zmk_rpc_hid_reports(framed_bytes: &[u8]) -> Vec<[u8; RAW_REPORT_SIZE]> {
    framed_bytes
        .chunks(ZMK_RPC_HID_MAX_PAYLOAD)
        .map(|chunk| {
            let mut report = [0u8; RAW_REPORT_SIZE];
            report[0] = ZMK_RPC_HID_TAG;
            report[1] = chunk.len() as u8;
            report[2..2 + chunk.len()].copy_from_slice(chunk);
            report
        })
        .collect()
}

/// Shared queue and async waker for incoming VIA RPC response packets.
#[derive(Default, Debug)]
pub struct ResponseSlot {
    pending: VecDeque<Vec<u8>>,
    waker: Option<Waker>,
}

impl ResponseSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueues a response packet and wakes any task waiting for it.
    pub fn push_response(&mut self, packet: Vec<u8>) {
        self.pending.push_back(packet);
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }

    /// Takes the next queued response packet if available.
    pub fn pop_response(&mut self) -> Option<Vec<u8>> {
        self.pending.pop_front()
    }
}

/// A future that resolves when a VIA RPC response packet is received.
pub struct ResponseFuture {
    slot: Arc<Mutex<ResponseSlot>>,
}

impl Future for ResponseFuture {
    type Output = Result<Vec<u8>, DeviceError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut guard = self.slot.lock().unwrap();
        if let Some(packet) = guard.pop_response() {
            Poll::Ready(Ok(packet))
        } else {
            guard.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

/// Demuxes an incoming raw HID packet, supporting QMK/VIA responses, KeyPeek companion
/// telemetry (`0xFF` layer changes, `0xF1` key presses), and ZMK Studio RPC chunks (`0xF2`).
pub fn demux_input_report_with_zmk(
    bytes: &[u8],
    event_tx: &mpsc::Sender<DeviceEvent>,
    response_slot: &Mutex<ResponseSlot>,
    zmk_rpc: &ZmkRpcMux,
    ui_wake: &UiWake,
) {
    let stripped = if bytes.first() == Some(&0)
        && bytes.len() > 1
        && matches!(bytes.get(1), Some(&0xFF | &0xF1 | &ZMK_RPC_HID_TAG))
    {
        &bytes[1..]
    } else {
        bytes
    };

    if stripped.first() == Some(&ZMK_RPC_HID_TAG) && stripped.len() >= 2 {
        let len = (stripped[1] as usize).min(stripped.len() - 2);
        zmk_rpc.push_bytes(&stripped[2..2 + len], event_tx, ui_wake);
    } else if let Some(device_event) = decode_raw_hid_packet(bytes) {
        let _ = event_tx.send(device_event);
        ui_wake.request_repaint();
    } else {
        response_slot.lock().unwrap().push_response(bytes.to_vec());
    }
}

/// Demuxes an incoming raw HID packet (without ZMK RPC state).
pub fn demux_input_report(
    bytes: &[u8],
    event_tx: &mpsc::Sender<DeviceEvent>,
    response_slot: &Mutex<ResponseSlot>,
    ui_wake: &UiWake,
) {
    let zmk_rpc = ZmkRpcMux::new();
    demux_input_report_with_zmk(bytes, event_tx, response_slot, &zmk_rpc, ui_wake);
}

/// WebHID transport for QMK/VIA and ZMK Raw-HID keyboards running in browser WebAssembly builds.
#[cfg(target_arch = "wasm32")]
pub struct WebHidTransport {
    device: web_sys::HidDevice,
    event_rx: Mutex<Option<mpsc::Receiver<DeviceEvent>>>,
    response_slot: Arc<Mutex<ResponseSlot>>,
    zmk_rpc: Arc<ZmkRpcMux>,
    command_lock: futures_util::lock::Mutex<()>,
    inputreport_closure: Option<Closure<dyn FnMut(web_sys::HidInputReportEvent)>>,
}

#[cfg(target_arch = "wasm32")]
unsafe impl Send for WebHidTransport {}
#[cfg(target_arch = "wasm32")]
unsafe impl Sync for WebHidTransport {}

#[cfg(target_arch = "wasm32")]
impl WebHidTransport {
    /// Creates a new `WebHidTransport` wrapping an opened `web_sys::HidDevice` using an existing event channel.
    pub fn new_with_tx(
        device: web_sys::HidDevice,
        event_tx: mpsc::Sender<DeviceEvent>,
        ui_wake: UiWake,
    ) -> Result<Self, DeviceError> {
        let response_slot = Arc::new(Mutex::new(ResponseSlot::new()));
        let zmk_rpc = Arc::new(ZmkRpcMux::new());

        let slot_for_closure = Arc::clone(&response_slot);
        let zmk_for_closure = Arc::clone(&zmk_rpc);
        let wake_for_closure = ui_wake;
        let closure = Closure::<dyn FnMut(web_sys::HidInputReportEvent)>::new(
            move |event: web_sys::HidInputReportEvent| {
                let buffer = super::web::data_view_to_vec(&event.data());
                demux_input_report_with_zmk(
                    &buffer,
                    &event_tx,
                    &slot_for_closure,
                    &zmk_for_closure,
                    &wake_for_closure,
                );
            },
        );

        device
            .add_event_listener_with_callback("inputreport", closure.as_ref().unchecked_ref())
            .map_err(|e| {
                DeviceError::Transport(format!("Failed to attach inputreport listener: {e:?}"))
            })?;

        Ok(Self {
            device,
            event_rx: Mutex::new(None),
            response_slot,
            zmk_rpc,
            command_lock: futures_util::lock::Mutex::new(()),
            inputreport_closure: Some(closure),
        })
    }

    /// Creates a new `WebHidTransport` wrapping an opened `web_sys::HidDevice`.
    pub fn new(device: web_sys::HidDevice, ui_wake: UiWake) -> Result<Self, DeviceError> {
        let (event_tx, event_rx) = mpsc::channel();
        let mut transport = Self::new_with_tx(device, event_tx, ui_wake)?;
        transport.event_rx = Mutex::new(Some(event_rx));
        Ok(transport)
    }

    /// Opens the underlying `web_sys::HidDevice` if not already opened and creates a `WebHidTransport`.
    pub async fn open(
        device: web_sys::HidDevice,
        event_tx: mpsc::Sender<DeviceEvent>,
        ui_wake: UiWake,
    ) -> Result<Self, DeviceError> {
        if !device.opened() {
            wasm_bindgen_futures::JsFuture::from(device.open())
                .await
                .map_err(|e| {
                    DeviceError::Transport(format!("Failed to open WebHID device: {e:?}"))
                })?;
        }
        Self::new_with_tx(device, event_tx, ui_wake)
    }

    /// Attempts to find and open an already-paired WebHID device matching the specified VID and PID.
    pub async fn open_paired(
        vid: u16,
        pid: u16,
        event_tx: mpsc::Sender<DeviceEvent>,
        ui_wake: UiWake,
    ) -> Option<Self> {
        let dev = find_already_paired_hid_device(vid, pid).await?;
        Self::open(dev, event_tx, ui_wake).await.ok()
    }

    /// Attempts to find and open an already-paired WebHID device matching a BLE device name.
    pub async fn open_paired_by_name(
        ble_name: &str,
        event_tx: mpsc::Sender<DeviceEvent>,
        ui_wake: UiWake,
    ) -> Option<Self> {
        let dev = find_already_paired_hid_device_by_name(ble_name).await?;
        Self::open(dev, event_tx, ui_wake).await.ok()
    }

    /// Access the underlying `web_sys::HidDevice`.
    pub fn device(&self) -> &web_sys::HidDevice {
        &self.device
    }

    /// Claims the receiver for demuxed `DeviceEvent`s (layers changed, key pressed).
    pub fn take_event_receiver(&self) -> Result<mpsc::Receiver<DeviceEvent>, DeviceError> {
        self.event_rx
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| DeviceError::Transport("Event receiver already claimed".into()))
    }

    /// Pads `data` to [`RAW_REPORT_SIZE`] and returns a `js_sys::Uint8Array`.
    fn padded_report_array(data: &[u8]) -> js_sys::Uint8Array {
        let mut padded = [0u8; RAW_REPORT_SIZE];
        let copy_len = data.len().min(RAW_REPORT_SIZE);
        padded[..copy_len].copy_from_slice(&data[..copy_len]);
        let arr = js_sys::Uint8Array::new_with_length(RAW_REPORT_SIZE as u32);
        arr.copy_from(&padded);
        arr
    }

    /// Sends an output report without awaiting resolution (for non-blocking writes).
    pub fn fire_and_forget_report(&self, data: &[u8]) {
        let uint8_array = Self::padded_report_array(data);
        if let Ok(promise) = self.device.send_report_with_u8_array(0, &uint8_array) {
            wasm_bindgen_futures::spawn_local(async move {
                let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
            });
        }
    }

    /// Asynchronously sends a raw output report (report ID 0) to the device.
    pub async fn send_report(&self, data: &[u8]) -> Result<(), DeviceError> {
        let uint8_array = Self::padded_report_array(data);
        let promise = self
            .device
            .send_report_with_u8_array(0, &uint8_array)
            .map_err(|e| {
                DeviceError::Transport(format!("WebHID send_report call failed: {e:?}"))
            })?;
        wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(|e| DeviceError::Transport(format!("WebHID send_report failed: {e:?}")))?;
        Ok(())
    }

    /// Sends a ZMK Studio RPC request tunneled over Raw HID (`0xF2` chunks) and awaits the response.
    pub async fn call_zmk(
        &self,
        subsystem: studio::request::Subsystem,
    ) -> Result<studio::RequestResponse, DeviceError> {
        self.zmk_rpc
            .call(subsystem, 3, 1500, |bytes| async move {
                for report in encode_zmk_rpc_hid_reports(&bytes) {
                    self.send_report(&report).await?;
                }
                Ok(())
            })
            .await
    }

    /// Asynchronously awaits the next VIA RPC response packet.
    pub fn read_response(&self) -> ResponseFuture {
        ResponseFuture {
            slot: Arc::clone(&self.response_slot),
        }
    }

    /// Sends a VIA command and awaits the corresponding response with a timeout in milliseconds.
    pub async fn command_timeout(
        &self,
        command_id: u8,
        payload: &[u8],
        timeout_ms: i32,
    ) -> Result<Vec<u8>, DeviceError> {
        let _guard = self.command_lock.lock().await;
        self.response_slot.lock().unwrap().pending.clear();

        let mut report = [0u8; RAW_REPORT_SIZE];
        report[0] = command_id;
        let copy_len = payload.len().min(RAW_REPORT_SIZE - 1);
        report[1..1 + copy_len].copy_from_slice(&payload[..copy_len]);

        self.send_report(&report).await?;

        use futures_util::future::{select, Either};
        match select(
            Box::pin(self.read_response()),
            Box::pin(super::web::sleep_ms(timeout_ms)),
        )
        .await
        {
            Either::Left((resp, _)) => resp,
            Either::Right(_) => Err(DeviceError::Transport(format!(
                "Timed out waiting for response to command 0x{command_id:02X}"
            ))),
        }
    }

    /// Sends a VIA command and awaits the corresponding response (with a 1000ms timeout).
    pub async fn command(&self, command_id: u8, payload: &[u8]) -> Result<Vec<u8>, DeviceError> {
        self.command_timeout(command_id, payload, 1000).await
    }

    /// Closes the WebHID transport, removing the `inputreport` listener and closing the underlying device.
    pub fn close(&mut self) {
        self.zmk_rpc.clear_pending();
        if let Some(closure) = self.inputreport_closure.take() {
            let _ = self.device.remove_event_listener_with_callback(
                "inputreport",
                closure.as_ref().unchecked_ref(),
            );
        }
        if self.device.opened() {
            let promise = self.device.close();
            wasm_bindgen_futures::spawn_local(async move {
                let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
            });
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for WebHidTransport {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(target_arch = "wasm32")]
impl qmk_via_api::ViaTransport for WebHidTransport {
    fn write_report(&mut self, report: &[u8]) -> Result<(), qmk_via_api::Error> {
        let uint8_array = js_sys::Uint8Array::new_with_length(report.len() as u32);
        uint8_array.copy_from(report);
        let promise = self
            .device
            .send_report_with_u8_array(0, &uint8_array)
            .map_err(|e| qmk_via_api::Error::Hid(format!("{e:?}")))?;
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = wasm_bindgen_futures::JsFuture::from(promise).await {
                log::error!("WebHID write_report error: {e:?}");
            }
        });
        Ok(())
    }

    fn read_report(&mut self, _timeout_ms: Option<i32>) -> Result<Vec<u8>, qmk_via_api::Error> {
        self.response_slot
            .lock()
            .unwrap()
            .pop_response()
            .ok_or_else(|| qmk_via_api::Error::Hid("No pending WebHID response".into()))
    }
}

/// Returns whether the current browser environment supports the WebHID API.
#[cfg(target_arch = "wasm32")]
pub fn is_web_hid_supported() -> bool {
    super::web::has_navigator_property("hid")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn is_web_hid_supported() -> bool {
    false
}

#[cfg(target_arch = "wasm32")]
use crate::device_discovery::DiscoveredDevice;
#[cfg(target_arch = "wasm32")]
use crate::protocols::ConnectionSpec;

/// Prompts the user to select and pair a WebHID device matching QMK/VIA/KeyPeek usage page (0xFF60),
/// optionally filtered by vendor ID and product ID.
#[cfg(target_arch = "wasm32")]
pub async fn request_web_hid_device_with_filter(
    vid: Option<u16>,
    pid: Option<u16>,
) -> Result<Option<web_sys::HidDevice>, DeviceError> {
    let window =
        web_sys::window().ok_or_else(|| DeviceError::Transport("No window found".into()))?;
    if !is_web_hid_supported() {
        return Err(DeviceError::Unsupported(
            "WebHID is not supported by your browser. Please use Chrome, Edge or Opera.".into(),
        ));
    }

    let hid = window.navigator().hid();
    let filter = web_sys::HidDeviceFilter::new();
    filter.set_usage_page(0xFF60);
    if let Some(v) = vid {
        filter.set_vendor_id(v as u32);
    }
    if let Some(p) = pid {
        filter.set_product_id(p);
    }
    let filters = [filter];
    let options = web_sys::HidDeviceRequestOptions::new(&filters);

    let devices_val = match wasm_bindgen_futures::JsFuture::from(hid.request_device(&options)).await
    {
        Ok(val) => val,
        Err(err) => {
            if super::web::is_user_cancelled_picker(&err) {
                return Ok(None);
            }
            return Err(DeviceError::Transport(format!(
                "WebHID request error: {err:?}"
            )));
        }
    };

    let devices: js_sys::Array = devices_val
        .dyn_into()
        .map_err(|e| DeviceError::Transport(format!("Expected Array from requestDevice: {e:?}")))?;

    if devices.length() == 0 {
        return Ok(None);
    }

    let device: web_sys::HidDevice = devices
        .get(0)
        .dyn_into()
        .map_err(|e| DeviceError::Transport(format!("Expected HidDevice: {e:?}")))?;

    if !device.opened() {
        wasm_bindgen_futures::JsFuture::from(device.open())
            .await
            .map_err(|e| DeviceError::Transport(format!("Failed to open WebHID device: {e:?}")))?;
    }

    Ok(Some(device))
}

/// Prompts the user to select and pair a WebHID device matching QMK/VIA/KeyPeek usage page (0xFF60).
#[cfg(target_arch = "wasm32")]
pub async fn request_web_hid_device() -> Result<Option<web_sys::HidDevice>, DeviceError> {
    request_web_hid_device_with_filter(None, None).await
}

/// Searches already granted WebHID devices for the first device satisfying `predicate`.
#[cfg(target_arch = "wasm32")]
async fn find_paired_hid_device_matching(
    predicate: impl Fn(&web_sys::HidDevice) -> bool,
) -> Option<web_sys::HidDevice> {
    let window = web_sys::window()?;
    let devices_val = wasm_bindgen_futures::JsFuture::from(window.navigator().hid().get_devices())
        .await
        .ok()?;
    let devices: js_sys::Array = devices_val.dyn_into().ok()?;
    for i in 0..devices.length() {
        if let Ok(dev) = devices.get(i).dyn_into::<web_sys::HidDevice>() {
            if predicate(&dev) {
                return Some(dev);
            }
        }
    }
    None
}

/// Searches already granted WebHID devices for one matching the specified VID and PID.
#[cfg(target_arch = "wasm32")]
pub async fn find_already_paired_hid_device(vid: u16, pid: u16) -> Option<web_sys::HidDevice> {
    find_paired_hid_device_matching(|dev| dev.vendor_id() == vid && dev.product_id() == pid).await
}

/// Searches already granted WebHID devices for one matching the given BLE device name.
#[cfg(target_arch = "wasm32")]
pub async fn find_already_paired_hid_device_by_name(ble_name: &str) -> Option<web_sys::HidDevice> {
    find_paired_hid_device_matching(|dev| {
        crate::firmware::zmk::common::is_possible_ble_name_match(&dev.product_name(), ble_name)
    })
    .await
}

#[cfg(target_arch = "wasm32")]
pub use super::web::{sleep_ms, ConnectedWebDevice};

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
pub struct PendingZmkTelemetry {
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub event_tx: mpsc::Sender<DeviceEvent>,
}

#[cfg(target_arch = "wasm32")]
impl PendingZmkTelemetry {
    /// Prompts user to authorize the WebHID device and opens the transport.
    pub async fn request_and_open(self, ui_wake: UiWake) -> Option<WebHidTransport> {
        let dev = request_web_hid_device_with_filter(self.vid, self.pid)
            .await
            .ok()??;
        WebHidTransport::open(dev, self.event_tx, ui_wake)
            .await
            .ok()
    }
}

#[cfg(target_arch = "wasm32")]
pub enum WebConnectOutcome {
    Connected(ConnectedWebDevice),
    ZmkConnected {
        connected: ConnectedWebDevice,
        pending_telemetry: Option<PendingZmkTelemetry>,
    },
    RequiresLayoutFile {
        device: DiscoveredDevice,
        transport: Arc<WebHidTransport>,
    },
}

/// Prompts the user to pick a WebHID device, extracts `(vid, pid, base_name)`, and opens a [`WebHidTransport`].
#[cfg(target_arch = "wasm32")]
async fn open_picked_hid_device(
    default_prefix: &str,
    ui_wake: UiWake,
) -> Result<Option<(u16, u16, String, Arc<WebHidTransport>)>, DeviceError> {
    let Some(raw_device) = request_web_hid_device().await? else {
        return Ok(None);
    };
    let vid = raw_device.vendor_id();
    let pid = raw_device.product_id();
    let name = raw_device.product_name();
    let base_name = if name.trim().is_empty() {
        format!("{default_prefix} ({vid:04X}:{pid:04X})")
    } else {
        name
    };
    let transport = Arc::new(WebHidTransport::new(raw_device, ui_wake)?);
    Ok(Some((vid, pid, base_name, transport)))
}

/// Connects a ZMK keyboard over an already-opened [`WebHidTransport`] (using the `0xF2` RPC tunnel).
/// Both ZMK Studio RPC and live layer/key telemetry share the single WebHID connection.
#[cfg(target_arch = "wasm32")]
pub async fn connect_zmk_over_web_hid(
    transport: Arc<WebHidTransport>,
    vid: u16,
    pid: u16,
    base_name: String,
    overlay_config: OverlayConfig,
    ui_wake: UiWake,
) -> Result<WebConnectOutcome, DeviceError> {
    let event_rx = transport.take_event_receiver()?;
    crate::firmware::zmk::web::finish_zmk_web_connection(
        crate::firmware::zmk::web::WebZmkTransport::Hid(transport),
        base_name,
        vid,
        pid,
        "WebHID",
        crate::protocols::ZmkTransportConfig::Serial("WebHID".to_string()),
        event_rx,
        None,
        None,
        overlay_config,
        ui_wake,
    )
    .await
}

/// Prompts the user to select a ZMK keyboard via WebHID (USB or Bluetooth with KeyPeek module)
/// and connects ZMK Studio RPC + live telemetry over the single WebHID interface.
#[cfg(target_arch = "wasm32")]
pub async fn request_and_connect_zmk_hid(
    overlay_config: OverlayConfig,
    ui_wake: UiWake,
) -> Result<Option<WebConnectOutcome>, DeviceError> {
    let Some((vid, pid, base_name, transport)) =
        open_picked_hid_device("ZMK Device", ui_wake.clone()).await?
    else {
        return Ok(None);
    };
    connect_zmk_over_web_hid(transport, vid, pid, base_name, overlay_config, ui_wake)
        .await
        .map(Some)
}

/// Requests a WebHID device from user gesture, opens it, connects the live QMK/Vial protocol
/// (or automatically falls back to ZMK over WebHID if the device is a ZMK keyboard with the KeyPeek RPC bridge),
/// and returns the [`WebConnectOutcome`].
#[cfg(target_arch = "wasm32")]
pub async fn request_and_connect_device(
    overlay_config: OverlayConfig,
    ui_wake: UiWake,
) -> Result<Option<WebConnectOutcome>, DeviceError> {
    let Some((vid, pid, base_name, transport)) =
        open_picked_hid_device("USB Device", ui_wake.clone()).await?
    else {
        return Ok(None);
    };

    // ZMK keyboards use 0x1D50:0x615E by default; probe ZMK Studio directly to avoid Vial/VIA timeouts.
    if (vid, pid) == (0x1D50, 0x615E) {
        return connect_zmk_over_web_hid(transport, vid, pid, base_name, overlay_config, ui_wake)
            .await
            .map(Some);
    }

    let discovered = DiscoveredDevice {
        base_name: base_name.clone(),
        vid,
        pid,
        driver_id: "qmk",
        protocol_label: "WebHID",
        requires_layout_file: false,
        spec: ConnectionSpec::Vial { vid, pid },
    };

    match crate::firmware::qmk::web::connect_web_qmk(Arc::clone(&transport), vid, pid, None).await {
        Ok(crate::firmware::qmk::web::WebQmkOutcome::Connected(protocol)) => {
            let connected = ConnectedWebDevice::new(discovered, protocol, overlay_config, ui_wake)?;
            Ok(Some(WebConnectOutcome::Connected(connected)))
        }
        Ok(crate::firmware::qmk::web::WebQmkOutcome::RequiresLayoutFile) => {
            let mut dev = discovered;
            dev.requires_layout_file = true;
            dev.spec = ConnectionSpec::Via {
                vid,
                pid,
                json_path: String::new(),
            };
            Ok(Some(WebConnectOutcome::RequiresLayoutFile {
                device: dev,
                transport,
            }))
        }
        Err(qmk_err) => {
            log::info!(
                "Device did not respond to QMK/VIA ({qmk_err}); probing ZMK Studio over WebHID..."
            );
            match connect_zmk_over_web_hid(transport, vid, pid, base_name, overlay_config, ui_wake)
                .await
            {
                Ok(zmk_outcome) => Ok(Some(zmk_outcome)),
                Err(DeviceError::DeviceLocked) => Err(DeviceError::DeviceLocked),
                Err(_) => Err(qmk_err),
            }
        }
    }
}

/// Connects a VIA keyboard over WebHID using a user-provided layout JSON string.
#[cfg(target_arch = "wasm32")]
pub async fn connect_via_with_layout(
    device: DiscoveredDevice,
    transport: Arc<WebHidTransport>,
    layout_json: String,
    overlay_config: OverlayConfig,
    ui_wake: UiWake,
) -> Result<ConnectedWebDevice, DeviceError> {
    let vid = device.vid;
    let pid = device.pid;

    let outcome =
        crate::firmware::qmk::web::connect_web_qmk(transport, vid, pid, Some(&layout_json)).await?;

    match outcome {
        crate::firmware::qmk::web::WebQmkOutcome::Connected(protocol) => {
            ConnectedWebDevice::new(device, protocol, overlay_config, ui_wake)
        }
        crate::firmware::qmk::web::WebQmkOutcome::RequiresLayoutFile => Err(DeviceError::Protocol(
            "Layout definition is required".into(),
        )),
    }
}

#[cfg(target_arch = "wasm32")]
struct FilePickerState {
    input: web_sys::HtmlInputElement,
    on_change: Option<Closure<dyn FnMut()>>,
    on_cancel: Option<Closure<dyn FnMut()>>,
}

#[cfg(target_arch = "wasm32")]
impl FilePickerState {
    fn schedule_cleanup(state: &std::rc::Rc<std::cell::RefCell<Option<Self>>>) {
        let state_ref = std::rc::Rc::clone(state);
        wasm_bindgen_futures::spawn_local(async move {
            state_ref.borrow_mut().take();
        });
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for FilePickerState {
    fn drop(&mut self) {
        if let Some(on_change) = self.on_change.take() {
            let _ = self
                .input
                .remove_event_listener_with_callback("change", on_change.as_ref().unchecked_ref());
        }
        if let Some(on_cancel) = self.on_cancel.take() {
            let _ = self
                .input
                .remove_event_listener_with_callback("cancel", on_cancel.as_ref().unchecked_ref());
        }
    }
}

/// Opens the browser's native file chooser and sends (filename, text_content) to tx.
#[cfg(target_arch = "wasm32")]
pub fn trigger_web_file_picker(
    tx: mpsc::Sender<Result<(String, String), String>>,
    ui_wake: UiWake,
) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Ok(el) = document.create_element("input") else {
        return;
    };
    let Ok(input) = el.dyn_into::<web_sys::HtmlInputElement>() else {
        return;
    };

    input.set_type("file");
    let _ = input.set_attribute("accept", ".json,application/json");

    let state: std::rc::Rc<std::cell::RefCell<Option<FilePickerState>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));

    let input_clone = input.clone();
    let tx_clone = tx;
    let wake_change = ui_wake.clone();
    let state_for_change = std::rc::Rc::clone(&state);

    let on_change = Closure::<dyn FnMut()>::new(move || {
        FilePickerState::schedule_cleanup(&state_for_change);

        if let Some(file) = input_clone.files().and_then(|f| f.get(0)) {
            let filename = file.name();
            let promise = file.text();
            let tx_inner = tx_clone.clone();
            let wake_inner = wake_change.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match wasm_bindgen_futures::JsFuture::from(promise).await {
                    Ok(js_val) => {
                        if let Some(text) = js_val.as_string() {
                            let _ = tx_inner.send(Ok((filename, text)));
                        } else {
                            let _ = tx_inner.send(Err("Failed to read file as text".into()));
                        }
                    }
                    Err(e) => {
                        let _ = tx_inner.send(Err(format!("File read error: {e:?}")));
                    }
                }
                wake_inner.request_repaint();
            });
        }
    });

    let wake_cancel = ui_wake;
    let state_for_cancel = std::rc::Rc::clone(&state);
    let on_cancel = Closure::<dyn FnMut()>::new(move || {
        FilePickerState::schedule_cleanup(&state_for_cancel);
        wake_cancel.request_repaint();
    });

    if input
        .add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref())
        .is_err()
        || input
            .add_event_listener_with_callback("cancel", on_cancel.as_ref().unchecked_ref())
            .is_err()
    {
        return;
    }

    *state.borrow_mut() = Some(FilePickerState {
        input: input.clone(),
        on_change: Some(on_change),
        on_cancel: Some(on_cancel),
    });

    input.click();
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;
    use zmk_studio_api::framing::encode_frame;
    use zmk_studio_api::proto::zmk::studio::{self, RequestResponse, Response};

    #[test]
    fn test_demux_layer_state_packet() {
        let (event_tx, event_rx) = mpsc::channel();
        let slot = Mutex::new(ResponseSlot::new());
        let ui_wake = UiWake::new(Arc::new(|| {}));

        // 0xFF layer state packet: size=1, default=0, active=2
        let packet = [0xFF, 1, 0, 2];
        demux_input_report(&packet, &event_tx, &slot, &ui_wake);

        assert!(slot.lock().unwrap().pop_response().is_none());
        let event = event_rx.try_recv().expect("expected event");
        assert_eq!(
            event,
            DeviceEvent::LayersChanged {
                active_layers: 2,
                default_layers: 0,
            }
        );
    }

    #[test]
    fn test_demux_key_event_packet() {
        let (event_tx, event_rx) = mpsc::channel();
        let slot = Mutex::new(ResponseSlot::new());
        let ui_wake = UiWake::new(Arc::new(|| {}));

        // 0xF1 key event packet: row=1, col=4, pressed=1
        let packet = [0xF1, 1, 4, 1];
        demux_input_report(&packet, &event_tx, &slot, &ui_wake);

        assert!(slot.lock().unwrap().pop_response().is_none());
        let event = event_rx.try_recv().expect("expected event");
        assert_eq!(
            event,
            DeviceEvent::KeyPressed {
                row: 1,
                col: 4,
                pressed: true,
            }
        );
    }

    #[test]
    fn test_demux_via_rpc_response() {
        let (event_tx, event_rx) = mpsc::channel();
        let slot = Mutex::new(ResponseSlot::new());
        let ui_wake = UiWake::new(Arc::new(|| {}));

        // Standard VIA get_protocol_version response: [0x01, 0x00, 0x0C, ...]
        let mut packet = vec![0u8; RAW_REPORT_SIZE];
        packet[0] = 0x01;
        packet[1] = 0x00;
        packet[2] = 0x0C;
        demux_input_report(&packet, &event_tx, &slot, &ui_wake);

        assert!(event_rx.try_recv().is_err());
        let resp = slot
            .lock()
            .unwrap()
            .pop_response()
            .expect("expected VIA response");
        assert_eq!(resp[0], 0x01);
        assert_eq!(resp[2], 0x0C);
    }

    #[test]
    fn test_zmk_rpc_hid_chunk_encode_and_demux() {
        let (event_tx, event_rx) = mpsc::channel();
        let slot = Mutex::new(ResponseSlot::new());
        let zmk_rpc = ZmkRpcMux::new();
        let ui_wake = UiWake::new(Arc::new(|| {}));

        let (tx, mut rx) = futures_channel::oneshot::channel();
        zmk_rpc.register_pending(77, tx);

        // Create a large ZMK response that spans multiple 30-byte HID payload chunks
        let response = Response {
            r#type: Some(studio::response::Type::RequestResponse(RequestResponse {
                request_id: 77,
                subsystem: Some(studio::request_response::Subsystem::Behaviors(
                    zmk_studio_api::proto::zmk::behaviors::Response {
                        response_type: Some(
                            zmk_studio_api::proto::zmk::behaviors::response::ResponseType::GetBehaviorDetails(
                                zmk_studio_api::proto::zmk::behaviors::GetBehaviorDetailsResponse {
                                    id: 1,
                                    display_name: "Key Press Behavior With Long Display Name".to_string(),
                                    metadata: vec![],
                                },
                            ),
                        ),
                    },
                )),
            })),
        };

        let wire = encode_frame(&response.encode_to_vec());
        assert!(wire.len() > ZMK_RPC_HID_MAX_PAYLOAD);

        let reports = encode_zmk_rpc_hid_reports(&wire);
        assert!(reports.len() >= 2);

        for report in &reports {
            demux_input_report_with_zmk(report, &event_tx, &slot, &zmk_rpc, &ui_wake);
        }

        // Ensure it did not leak into VIA response_slot or telemetry event_rx
        assert!(slot.lock().unwrap().pop_response().is_none());
        assert!(event_rx.try_recv().is_err());

        let decoded = rx
            .try_recv()
            .expect("oneshot should not be canceled")
            .expect("expected decoded ZMK RequestResponse");
        assert_eq!(decoded.request_id, 77);
    }
}
