use std::future::Future;
use std::sync::mpsc;
use std::sync::Arc;

use keypeek_presentation::{
    github_link, keymap_editor::KEY_UNIT, version_link, KeyPeekAppCore, LegendMode, OverlayHost,
    SettingsStore,
};
use keypeek_protocol::connection_manager::{ConnectionEvent, ConnectionState};
use keypeek_protocol::platform::web::{ConnectedWebDevice, OverlayConfig};
use keypeek_protocol::platform::web_ble::{
    is_web_bluetooth_supported, is_windows_browser, request_and_connect_zmk_ble,
};
use keypeek_protocol::platform::web_hid::{
    connect_via_with_layout, is_web_hid_supported, request_and_connect_device,
    request_and_connect_zmk_hid, trigger_web_file_picker, PendingZmkTelemetry, WebConnectOutcome,
    WebHidTransport,
};
use keypeek_protocol::platform::web_serial::{is_web_serial_supported, request_and_connect_zmk};
use keypeek_protocol::{DeviceError, DiscoveredDevice, Keyboard, UiWake};

/// URL to the KeyPeek companion ZMK module required for Bluetooth connections on Windows.
const KEYPEEK_ZMK_MODULE_URL: &str = "https://github.com/srwi/zmk-keypeek-layer-notifier";

/// Background color for the browser canvas, matching egui's panel background.
pub fn clear_color(visuals: &egui::Visuals) -> egui::Rgba {
    visuals.panel_fill.into()
}

/// Top-level coordinator for browser execution: WebHID/WebSerial/WebBluetooth pairing,
/// single-canvas layout, responsive sidebar editor, and web local storage.
pub struct WebOverlayApp {
    pub core: KeyPeekAppCore,

    pub(crate) pairing_rx:
        Option<(mpsc::Receiver<Result<Option<WebConnectOutcome>, DeviceError>>, bool)>,
    pub(crate) pending_via: Option<(DiscoveredDevice, Arc<WebHidTransport>)>,
    pub(crate) web_file_rx: Option<mpsc::Receiver<Result<(String, String), String>>>,
    pub(crate) pending_zmk_telemetry: Option<PendingZmkTelemetry>,
    pub(crate) telemetry_hid: Option<WebHidTransport>,
    pub(crate) telemetry_rx: Option<mpsc::Receiver<Option<WebHidTransport>>>,
    pub(crate) selected_layer: Option<usize>,
    pub(crate) show_zmk_ble_windows_help_modal: bool,
}

pub type OverlayApp = WebOverlayApp;

impl std::ops::Deref for WebOverlayApp {
    type Target = KeyPeekAppCore;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl std::ops::DerefMut for WebOverlayApp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

impl WebOverlayApp {
    pub fn new(ui_wake: UiWake, settings_store: Arc<dyn SettingsStore>) -> Self {
        Self::with_devices(ui_wake, settings_store, Vec::new())
    }

    pub fn with_devices(
        ui_wake: UiWake,
        settings_store: Arc<dyn SettingsStore>,
        available_devices: Vec<DiscoveredDevice>,
    ) -> Self {
        Self {
            core: KeyPeekAppCore::new(ui_wake, settings_store, available_devices, false),
            pairing_rx: None,
            pending_via: None,
            web_file_rx: None,
            pending_zmk_telemetry: None,
            telemetry_hid: None,
            telemetry_rx: None,
            selected_layer: None,
            show_zmk_ble_windows_help_modal: false,
        }
    }

    /// Background color for browser mode, matching active theme visuals.
    pub fn clear_color(&self, visuals: &egui::Visuals) -> egui::Rgba {
        clear_color(visuals)
    }

    /// Returns the currently pinned layer (editor target or manual web selection),
    /// or `None` if following hardware (Auto mode).
    pub(crate) fn effective_pinned_layer(&self) -> Option<usize> {
        self.editor.pinned_layer().or(self.selected_layer)
    }

    /// Disconnects the active keyboard connection, tears down protocol session,
    /// and closes the keymap editor.
    pub fn disconnect_from_ui(&mut self) {
        self.core.disconnect_from_ui();
        self.clear_transient_web_state();
    }

    /// Clears transient pairing, telemetry, and pending file/device resources.
    fn clear_transient_web_state(&mut self) {
        self.pending_via = None;
        self.pending_zmk_telemetry = None;
        self.telemetry_hid = None;
        self.web_file_rx = None;
        self.show_zmk_ble_windows_help_modal = false;
    }

    /// Web update step: polls pairing channels, telemetry channels, and file picker channels.
    pub fn update(&mut self, ctx: &egui::Context, _host: &mut dyn OverlayHost) {
        if let Some(event) = self.core.update_connection(ctx) {
            match event {
                ConnectionEvent::Disconnected => {
                    self.clear_transient_web_state();
                }
                ConnectionEvent::RequiresLayoutFile(_) => {
                    self.trigger_web_layout_file_picker();
                }
                _ => {}
            }
        }

        if let Some((rx, show_ble_help_on_cancel)) = &self.pairing_rx {
            let show_ble_help_on_cancel = *show_ble_help_on_cancel;
            match rx.try_recv() {
                Ok(Ok(Some(WebConnectOutcome::Connected(connected)))) => {
                    self.handle_connected_web_device(connected);
                    self.pairing_rx = None;
                }
                Ok(Ok(Some(WebConnectOutcome::ZmkConnected {
                    connected,
                    pending_telemetry,
                }))) => {
                    self.handle_connected_web_device(connected);
                    self.pending_zmk_telemetry = pending_telemetry;
                    self.pairing_rx = None;
                }
                Ok(Ok(Some(WebConnectOutcome::RequiresLayoutFile { device, transport }))) => {
                    self.ui.settings_error = None;
                    self.ui.settings_warning = None;
                    self.connection_mgr.set_requires_layout_file(device.clone());
                    self.pending_via = Some((device, transport));
                    self.pairing_rx = None;
                }
                Ok(Ok(None)) => {
                    self.pairing_rx = None;
                    if self.connection_mgr.is_connecting() {
                        self.connection_mgr.disconnect();
                    }
                    if show_ble_help_on_cancel {
                        self.show_zmk_ble_windows_help_modal = true;
                    }
                }
                Ok(Err(err)) => {
                    self.connection_mgr.handle_device_error(&err);
                    self.pairing_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pairing_rx = None;
                    if self.connection_mgr.is_connecting() {
                        self.connection_mgr.disconnect();
                    }
                }
            }
        }

        if let Some(rx) = &self.telemetry_rx {
            match rx.try_recv() {
                Ok(Some(hid)) => {
                    self.telemetry_hid = Some(hid);
                    self.telemetry_rx = None;
                }
                Ok(None) => {
                    self.telemetry_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.telemetry_rx = None;
                }
            }
        }

        if let Some(rx) = &self.web_file_rx {
            match rx.try_recv() {
                Ok(Ok((_filename, content))) => {
                    self.web_file_rx = None;
                    self.connect_pending_via_with_json(content);
                }
                Ok(Err(e)) => {
                    self.ui.settings_error = Some(e);
                    self.web_file_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.web_file_rx = None;
                }
            }
        }
    }

    /// Completes connection when a web device connects.
    pub(super) fn handle_connected_web_device(&mut self, connected: ConnectedWebDevice) {
        let idx = self.connection_mgr.add_device(connected.device.clone());
        self.connection_mgr.select_device(idx);
        self.connection_mgr
            .set_connected(connected.keyboard, connected.profile, None);
        self.ui.clear_alerts();
        self.clear_transient_web_state();
    }

    /// Spawns a browser pairing task and routes its [`WebConnectOutcome`] into `self.pairing_rx`.
    fn spawn_pairing<F, Fut>(&mut self, connect_fn: F)
    where
        F: FnOnce(OverlayConfig, UiWake) -> Fut + 'static,
        Fut: Future<Output = Result<Option<WebConnectOutcome>, DeviceError>> + 'static,
    {
        self.spawn_pairing_inner(false, connect_fn);
    }

    fn spawn_pairing_inner<F, Fut>(&mut self, show_ble_help_on_cancel: bool, connect_fn: F)
    where
        F: FnOnce(OverlayConfig, UiWake) -> Fut + 'static,
        Fut: Future<Output = Result<Option<WebConnectOutcome>, DeviceError>> + 'static,
    {
        self.connection_mgr.set_connecting();
        let (tx, rx) = mpsc::channel();
        self.pairing_rx = Some((rx, show_ble_help_on_cancel));
        let ui_wake = self.connection_mgr.ui_wake().clone();
        let overlay_config = self.settings.overlay_config();
        wasm_bindgen_futures::spawn_local(async move {
            let result = connect_fn(overlay_config, ui_wake.clone()).await;
            let _ = tx.send(result);
            ui_wake.request_repaint();
        });
    }

    /// Prompts user to pair a WebHID device (QMK/Vial/VIA, or ZMK with KeyPeek module).
    pub fn request_web_hid_pairing(&mut self) {
        self.spawn_pairing(request_and_connect_device);
    }

    /// Prompts user to connect a ZMK Studio keyboard over Web Serial (USB).
    pub fn request_web_serial_pairing(&mut self) {
        self.spawn_pairing(request_and_connect_zmk);
    }

    /// Prompts user to connect a ZMK Studio keyboard over Bluetooth (uses WebHID on Windows/when WebBLE is unavailable, else Web Bluetooth GATT).
    pub fn request_web_ble_pairing(&mut self) {
        let on_windows = is_windows_browser();
        if is_web_hid_supported() && (on_windows || !is_web_bluetooth_supported()) {
            self.spawn_pairing_inner(on_windows, request_and_connect_zmk_hid);
        } else {
            self.spawn_pairing(request_and_connect_zmk_ble);
        }
    }

    /// Opens the browser file chooser for VIA layout JSON files.
    pub fn trigger_web_layout_file_picker(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.web_file_rx = Some(rx);
        let ui_wake = self.connection_mgr.ui_wake().clone();
        trigger_web_file_picker(tx, ui_wake);
    }

    /// Connects a pending VIA device with the selected layout JSON.
    pub fn connect_pending_via_with_json(&mut self, json_content: String) {
        let Some((device, transport)) = self.pending_via.take() else {
            return;
        };
        self.spawn_pairing(move |overlay_config, ui_wake| async move {
            connect_via_with_layout(device, transport, json_content, overlay_config, ui_wake)
                .await
                .map(|conn| Some(WebConnectOutcome::Connected(conn)))
        });
    }

    /// Draws the centered responsive keyboard layout.
    pub fn draw_overlay_canvas(&mut self, ui: &mut egui::Ui, keyboard: &Keyboard) {
        let (raw_w, raw_h) = keyboard.layout().get_dimensions();
        let layout_w = raw_w.max(1.0);
        let layout_h = raw_h.max(1.0);
        const PADDING: f32 = 32.0;
        const MAX_KEY_SIZE: f32 = 75.0;
        const MIN_KEY_SIZE: f32 = 18.0;

        let avail_size = ui.available_size();
        let avail_w = (avail_size.x - PADDING).max(MIN_KEY_SIZE);
        let avail_h = (avail_size.y - PADDING).max(MIN_KEY_SIZE);

        let key_size = (avail_w / layout_w)
            .min(avail_h / layout_h)
            .clamp(MIN_KEY_SIZE, MAX_KEY_SIZE);

        let overlay_h = layout_h * key_size;
        let v_space = ((avail_size.y - overlay_h) * 0.5).max(0.0);

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if v_space > 0.0 {
                    ui.add_space(v_space);
                }

                ui.vertical_centered(|ui| {
                    let pinned_layer = self.effective_pinned_layer();
                    self.core
                        .overlay_view(keyboard, key_size, true)
                        .with_pinned_layer(pinned_layer)
                        .show(ui);
                });

                if let Some(target) = self.editor.target {
                    self.selected_layer = Some(target.layer_index);
                }
            });
    }

    /// Draws the centered QMK/Vial (left) and ZMK (right) firmware connection cards.
    fn render_connect_cards(&mut self, ui: &mut egui::Ui) {
        const CARD_GAP: f32 = 18.0;

        let v_pad = ((ui.available_height() - (CARD_HEIGHT + 65.0)) * 0.45).max(20.0);
        ui.add_space(v_pad);

        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("Connect a keyboard to view and edit layers")
                    .size(15.0)
                    .weak(),
            );
            ui.add_space(20.0);

            let total_row_width = CARD_WIDTH * 2.0 + CARD_GAP;
            let left_space = ((ui.available_width() - total_row_width) * 0.5).max(0.0);

            let hid_supported = is_web_hid_supported();
            let serial_supported = is_web_serial_supported();
            let ble_enabled = is_web_bluetooth_supported() || hid_supported;
            let usb_label = format!("{} USB", egui_phosphor::regular::USB);
            let ble_label = format!("{} Bluetooth", egui_phosphor::regular::BLUETOOTH);
            let ble_hover = if is_windows_browser() {
                "Connect ZMK keyboard over Bluetooth (requires KeyPeek module on Windows)"
            } else {
                "Connect ZMK Studio keyboard over Bluetooth"
            };

            ui.horizontal(|ui| {
                if left_space > 0.0 {
                    ui.add_space(left_space);
                }

                render_firmware_card(ui, "QMK / Vial", "Vial & VIA keyboards", |ui, btn_w| {
                    if connect_button(
                        ui,
                        btn_w,
                        hid_supported,
                        &usb_label,
                        "Connect QMK, Vial, or VIA keyboard over USB",
                        "WebHID",
                    ) {
                        self.request_web_hid_pairing();
                    }
                });

                ui.add_space(CARD_GAP - ui.spacing().item_spacing.x * 2.0);

                render_firmware_card(ui, "ZMK", "ZMK Studio keyboards", |ui, btn_w| {
                    ui.horizontal(|ui| {
                        if connect_button(
                            ui,
                            btn_w,
                            serial_supported,
                            &usb_label,
                            "Connect ZMK Studio keyboard over USB",
                            "Web Serial",
                        ) {
                            self.request_web_serial_pairing();
                        }

                        if connect_button(
                            ui,
                            btn_w,
                            ble_enabled,
                            &ble_label,
                            ble_hover,
                            "Web Bluetooth / WebHID",
                        ) {
                            self.request_web_ble_pairing();
                        }
                    });
                });
            });
        });
    }

    /// Renders the web interface: top bar, editor sidebar, overlay canvas, and modals.
    pub fn render_web(&mut self, ui: &mut egui::Ui) {
        let connected = self.connection_mgr.connected_pair();
        let keyboard_ref = connected.as_ref().map(|(k, _)| k.as_ref());

        // 1. Fixed top panel for connection status and quick settings
        self.render_web_top_bar(ui, keyboard_ref);

        // 2. Resizable left sidebar for keymap editor (if open and connected)
        if let Some((keyboard, profile)) = &connected {
            self.core
                .draw_editor_sidebar(ui, keyboard, profile.as_ref());
        }

        // 3. Bottom panel of the main panel for layer buttons and legend mode
        if let Some(keyboard) = keyboard_ref {
            self.render_web_bottom_bar(ui, keyboard);
        }

        // 4. Central panel for the keyboard overlay canvas or connection cards
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some((keyboard, _)) = &connected {
                self.draw_overlay_canvas(ui, keyboard);
            } else {
                self.render_connect_cards(ui);
            }
        });

        // 5. Modal prompts
        let ctx = ui.ctx().clone();
        self.render_web_layout_modal(&ctx);
        self.render_zmk_telemetry_modal(&ctx);
        self.render_zmk_ble_windows_help_modal(&ctx);
    }

    /// Bottom panel of the main panel: centered layer buttons and bottom-right legend mode.
    fn render_web_bottom_bar(&mut self, ui: &mut egui::Ui, kbd: &Keyboard) {
        egui::Panel::bottom("web_bottom_bar")
            .resizable(false)
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.add_space(3.0);
                const ROW_HEIGHT: f32 = 24.0;
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), ROW_HEIGHT),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.spacing_mut().interact_size.y = 22.0;
                        let total_width = ui.available_width();

                        // Centered layer buttons
                        let count = kbd.layer_infos().len();
                        let spacing = ui.spacing().item_spacing.x;
                        let is_editing = self.editor.is_open();

                        const AUTO_WIDTH: f32 = 46.0;
                        const LAYER_BTN_WIDTH: f32 = 38.0;

                        let (btn_count, base_width) = if is_editing {
                            (count, count as f32 * LAYER_BTN_WIDTH)
                        } else {
                            (count + 1, AUTO_WIDTH + count as f32 * LAYER_BTN_WIDTH)
                        };
                        let center_width =
                            base_width + btn_count.saturating_sub(1) as f32 * spacing;

                        let left_space = ((total_width - center_width) * 0.5)
                            .min(total_width - center_width - 170.0)
                            .max(0.0);

                        if left_space > 0.0 {
                            ui.add_space(left_space);
                        }

                        let selected_layer = self.effective_pinned_layer();
                        let style = self.settings.paint_style(KEY_UNIT);

                        if !is_editing {
                            let is_auto = self.selected_layer.is_none();
                            let auto_btn = keypeek_presentation::keymap_editor::auto_button(
                                ui, is_auto, AUTO_WIDTH, &style,
                            );

                            if auto_btn.clicked() {
                                self.selected_layer = None;
                            }
                        }

                        let clicked = keypeek_presentation::keymap_editor::draw_layer_switcher(
                            ui,
                            kbd,
                            selected_layer,
                            Some(LAYER_BTN_WIDTH),
                            &style,
                        );

                        if let Some(layer) = clicked {
                            if is_editing {
                                self.editor.select_layer(kbd, layer);
                                self.selected_layer = Some(layer);
                            } else if self.selected_layer == Some(layer) {
                                // Toggle: clicking the already selected/pinned layer returns to Auto
                                self.selected_layer = None;
                            } else {
                                self.selected_layer = Some(layer);
                            }
                        }

                        // Render right-aligned legends selector
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            egui::ComboBox::from_id_salt("web_legend_mode")
                                .selected_text(self.settings.draft.legend_mode.to_string())
                                .show_ui(ui, |ui| {
                                    for mode in LegendMode::ALL {
                                        ui.selectable_value(
                                            &mut self.settings.draft.legend_mode,
                                            mode,
                                            mode.to_string(),
                                        );
                                    }
                                });
                            ui.label("Legends:");
                        });
                    },
                );
                ui.add_space(3.0);
            });

        self.sync_visual_settings();
    }

    /// Top bar with title, layout switcher, and connected status.
    fn render_web_top_bar(&mut self, ui: &mut egui::Ui, keyboard: Option<&Keyboard>) {
        egui::Panel::top("web_top_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("KeyPeek").strong().size(15.0));
                version_link(ui);
                ui.separator();
                github_link(ui);

                // Layout switching (shown when multiple layouts exist).
                if let Some(kbd) = keyboard {
                    if kbd.supports_multiple_layouts() {
                        ui.separator();
                        let current_layout = kbd.active_layout_name();
                        ui.label("Layout:");
                        egui::ComboBox::from_id_salt("web_layout_combo")
                            .selected_text(&current_layout)
                            .show_ui(ui, |ui| {
                                for name in kbd.layout_names() {
                                    let is_selected = name == current_layout;
                                    if ui.selectable_label(is_selected, &name).clicked()
                                        && !is_selected
                                    {
                                        self.switch_layout(&name);
                                    }
                                }
                            });
                    }
                }

                // Connection status and alerts on the right.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(_kbd) = keyboard {
                        if ui.button("Disconnect").clicked() {
                            self.disconnect_from_ui();
                        }
                        let dev_name = self
                            .connection_mgr
                            .selected_device()
                            .map(|d| d.display_name())
                            .unwrap_or_else(|| "Connected".to_string());
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {}",
                                egui_phosphor::regular::KEYBOARD,
                                dev_name
                            ))
                            .color(egui::Color32::LIGHT_GREEN),
                        );
                    }

                    if let ConnectionState::RequiresLayoutFile { device } =
                        self.connection_mgr.state()
                    {
                        if ui
                            .button(egui::RichText::new(format!(
                                "{} Load Layout for {}",
                                egui_phosphor::regular::FOLDER_OPEN,
                                device.base_name
                            )))
                            .clicked()
                        {
                            self.trigger_web_layout_file_picker();
                        }
                    }

                    if let ConnectionState::Locked { device } = self.connection_mgr.state() {
                        let msg =
                            format!("{} is locked. Unlock it on the keyboard.", device.base_name);
                        if render_alert_chip(ui, &msg, "🔒", egui::Color32::KHAKI) {
                            self.connection_mgr.disconnect();
                        }
                    } else if let ConnectionState::Error(err) = self.connection_mgr.state() {
                        if render_alert_chip(ui, err, "!", egui::Color32::LIGHT_RED) {
                            self.connection_mgr.clear_error();
                        }
                    } else if let Some(err) = &self.ui.settings_error {
                        if render_alert_chip(ui, err, "!", egui::Color32::LIGHT_RED) {
                            self.ui.settings_error = None;
                        }
                    } else if let Some(notice) = &self.ui.settings_warning {
                        if render_alert_chip(ui, notice, "i", egui::Color32::KHAKI) {
                            self.ui.settings_warning = None;
                        }
                    }
                });
            });
        });

        self.sync_visual_settings();
    }

    /// Modal prompt asking for VIA layout JSON.
    fn render_web_layout_modal(&mut self, ctx: &egui::Context) {
        let ConnectionState::RequiresLayoutFile { device } = self.connection_mgr.state().clone()
        else {
            return;
        };

        let mut trigger_picker = false;
        let mut dismiss = false;

        egui::Modal::new(egui::Id::new("web_layout_modal")).show(ctx, |ui| {
            ui.set_max_width(400.0);
            ui.heading(format!("Layout Required for {}", device.base_name));
            ui.add_space(8.0);
            ui.label(
                "This keyboard uses the VIA protocol. Please select its layout definition JSON file to complete connection.",
            );

            if let Some(err) = &self.ui.settings_error {
                ui.add_space(4.0);
                ui.colored_label(egui::Color32::LIGHT_RED, format!("Error: {err}"));
            }

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .button(egui::RichText::new(format!(
                        "{} Select Layout JSON",
                        egui_phosphor::regular::FOLDER_OPEN
                    )))
                    .clicked()
                {
                    trigger_picker = true;
                }

                if ui.button("Cancel").clicked() {
                    dismiss = true;
                }
            });
        });

        if trigger_picker {
            self.trigger_web_layout_file_picker();
        }
        if dismiss {
            self.disconnect_from_ui();
        }
    }

    /// Modal prompt asking to authorize WebHID companion telemetry for ZMK.
    fn render_zmk_telemetry_modal(&mut self, ctx: &egui::Context) {
        if self.pending_zmk_telemetry.is_none() {
            return;
        }

        let mut should_authorize = false;
        let mut should_skip = false;

        egui::Modal::new(egui::Id::new("zmk_telemetry_modal")).show(ctx, |ui| {
            ui.set_max_width(400.0);
            ui.heading("ZMK Connected");
            ui.add_space(8.0);
            ui.label(
                "To enable real-time layer switching and live key highlights, authorize the keyboard's HID telemetry interface.",
            );
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .button(egui::RichText::new(format!(
                        "{} Authorize Telemetry",
                        egui_phosphor::regular::BROADCAST
                    )))
                    .clicked()
                {
                    should_authorize = true;
                }
                if ui.button("Skip").clicked() {
                    should_skip = true;
                }
            });
        });

        if should_skip {
            self.pending_zmk_telemetry = None;
        } else if should_authorize {
            let pending = self.pending_zmk_telemetry.take().unwrap();
            let ui_wake = self.connection_mgr.ui_wake().clone();
            let (tx, rx) = mpsc::channel();
            self.telemetry_rx = Some(rx);

            wasm_bindgen_futures::spawn_local(async move {
                let hid = pending.request_and_open(ui_wake.clone()).await;
                let _ = tx.send(hid);
                ui_wake.request_repaint();
            });
        }
    }

    /// Modal shown on Windows only if the user cancels the ZMK Bluetooth picker (e.g. when their
    /// keyboard is not listed because it does not have the KeyPeek companion module flashed).
    fn render_zmk_ble_windows_help_modal(&mut self, ctx: &egui::Context) {
        if !self.show_zmk_ble_windows_help_modal {
            return;
        }

        let mut connect_usb = false;
        let mut close_modal = false;

        egui::Modal::new(egui::Id::new("zmk_ble_windows_help_modal")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading("ZMK Bluetooth on Windows");
            ui.add_space(8.0);
            ui.label(
                "Windows blocks standard Web Bluetooth access to keyboards that are already paired in Windows settings.",
            );
            ui.add_space(6.0);
            ui.label(
                "To connect a ZMK keyboard over Bluetooth on Windows, your firmware must include the KeyPeek companion module:",
            );
            ui.add_space(4.0);
            ui.add(
                egui::Hyperlink::from_label_and_url(
                    egui::RichText::new(format!(
                        "{} srwi/zmk-keypeek-layer-notifier",
                        egui_phosphor::regular::GITHUB_LOGO
                    )),
                    KEYPEEK_ZMK_MODULE_URL,
                )
                .open_in_new_tab(true),
            );
            ui.add_space(8.0);
            ui.label(
                "Without the module, you can connect your ZMK keyboard over USB instead:",
            );
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if is_web_serial_supported()
                    && ui
                        .button(egui::RichText::new(format!(
                            "{} Connect via USB",
                            egui_phosphor::regular::USB
                        )))
                        .clicked()
                {
                    connect_usb = true;
                }
                if ui.button("Close").clicked() {
                    close_modal = true;
                }
            });
        });

        if connect_usb {
            self.show_zmk_ble_windows_help_modal = false;
            self.request_web_serial_pairing();
        } else if close_modal {
            self.show_zmk_ble_windows_help_modal = false;
        }
    }

    /// Primary entry point called by the web platform runner.
    pub fn ui(&mut self, ui: &mut egui::Ui, host: &mut dyn OverlayHost) {
        let ctx = ui.ctx().clone();
        self.update(&ctx, host);
        self.render_web(ui);
        self.schedule_overlay_repaint(&ctx);
    }
}

const CARD_WIDTH: f32 = 225.0;
const CARD_HEIGHT: f32 = 115.0;
const BTN_HEIGHT: f32 = 30.0;

/// Renders a firmware connection card with a centered title, subtitle, and button area.
fn render_firmware_card(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    add_buttons: impl FnOnce(&mut egui::Ui, f32),
) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(18, 16))
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.set_width(CARD_WIDTH - 36.0);
            ui.set_min_height(CARD_HEIGHT - 32.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(title).strong().size(16.0));
                ui.label(egui::RichText::new(subtitle).size(12.0).weak());
                ui.add_space(14.0);
                let btn_w = (ui.available_width() - ui.spacing().item_spacing.x) * 0.5;
                add_buttons(ui, btn_w);
            });
        });
}

/// Renders a connection button with standardized sizing and unsupported-browser tooltip handling.
fn connect_button(
    ui: &mut egui::Ui,
    width: f32,
    enabled: bool,
    label: &str,
    hover_text: &str,
    unsupported_api: &str,
) -> bool {
    let btn = ui.add_enabled(
        enabled,
        egui::Button::new(label).min_size(egui::vec2(width, BTN_HEIGHT)),
    );
    if !enabled {
        btn.on_hover_text(format!(
            "{unsupported_api} is not supported in this browser. Please use Chrome, Edge, or Opera."
        ));
        false
    } else {
        btn.on_hover_text(hover_text).clicked()
    }
}

/// Renders a clickable chip in the top bar to dismiss notifications or errors.
fn render_alert_chip(ui: &mut egui::Ui, text: &str, icon: &str, color: egui::Color32) -> bool {
    ui.separator();
    ui.button(egui::RichText::new(format!("{icon} {text} x")).color(color))
        .on_hover_text("Click to dismiss")
        .clicked()
}
