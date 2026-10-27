use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::app_core::KeyPeekAppCore;
use crate::overlay_host::OverlayHost;
use crate::settings::SettingsStore;
use keypeek_protocol::connection_manager::{ConnectOutcome, ConnectionEvent};
use keypeek_protocol::{DiscoveredDevice, Keyboard, UiWake};

/// Background clear color for desktop: transparent overlay or dimmed backdrop.
pub fn clear_color(is_any_window_open: bool) -> egui::Rgba {
    if is_any_window_open {
        egui::Rgba::from_black_alpha(0.65)
    } else {
        egui::Rgba::TRANSPARENT
    }
}

/// Top-level coordinator for desktop execution: multi-windowing, system tray,
/// mouse passthrough, and native file dialogs.
pub struct DesktopOverlayApp {
    pub(super) settings_requested: Arc<AtomicBool>,
    pub core: KeyPeekAppCore,
    pub(super) file_dialog: egui_file_dialog::FileDialog,
    pub(super) available_monitors: Vec<String>,
    pub(super) mouse_passthrough: Option<bool>,
}

pub type DesktopApp = DesktopOverlayApp;
pub type OverlayApp = DesktopOverlayApp;

impl std::ops::Deref for DesktopOverlayApp {
    type Target = KeyPeekAppCore;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl std::ops::DerefMut for DesktopOverlayApp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

impl DesktopOverlayApp {
    pub fn active_monitor(&self) -> &crate::settings::MonitorSelection {
        &self.settings.active.monitor
    }

    pub fn set_available_monitors(&mut self, names: Vec<String>) {
        if self.available_monitors != names {
            self.available_monitors = names;
        }
    }

    pub fn new(
        settings_requested: Arc<AtomicBool>,
        ui_wake: UiWake,
        settings_store: Arc<dyn SettingsStore>,
        available_devices: Vec<DiscoveredDevice>,
    ) -> Self {
        Self {
            settings_requested,
            core: KeyPeekAppCore::new(ui_wake, settings_store, available_devices, true),
            file_dialog: egui_file_dialog::FileDialog::new(),
            available_monitors: Vec::new(),
            mouse_passthrough: None,
        }
    }

    /// Background color before egui paints.
    pub fn clear_color(&self) -> egui::Rgba {
        clear_color(self.is_any_window_open())
    }

    /// Initiates connection for the currently selected device in connection_mgr.
    pub fn connect_from_ui(&mut self) {
        if self.core.connect_selected_device() == ConnectOutcome::RequiresLayoutFile {
            self.pick_layout_file();
        }
    }

    /// Disconnects the active keyboard connection, tears down protocol session,
    /// and closes the keymap editor.
    pub fn disconnect_from_ui(&mut self) {
        self.core.disconnect_from_ui();
    }

    pub(super) fn get_anchor_params(&self) -> (egui::Align2, egui::Vec2) {
        self.settings
            .active
            .position
            .anchor_and_offset(self.settings.active.margin as f32)
    }

    pub(super) fn overlay_visible(&self) -> bool {
        self.core.is_overlay_visible()
    }

    /// Opens the native file dialog to pick a layout file.
    pub(super) fn pick_layout_file(&mut self) {
        self.file_dialog.pick_file();
    }

    /// Sets OS mouse passthrough mode based on current window state.
    pub(super) fn sync_mouse_passthrough(&mut self, host: &mut dyn OverlayHost) {
        let mouse_passthrough = !self.is_any_window_open();
        if self.mouse_passthrough == Some(mouse_passthrough) {
            return;
        }

        host.set_passthrough(mouse_passthrough);
        self.mouse_passthrough = Some(mouse_passthrough);
    }

    /// Updates state, handles background tasks, and processes input before drawing.
    pub fn update(&mut self, ctx: &egui::Context, host: &mut dyn OverlayHost) {
        if self.settings_requested.swap(false, Ordering::Relaxed) {
            self.core.ui.settings_visible = true;
        }

        if let Some(ConnectionEvent::RequiresLayoutFile(_)) = self.core.update_connection(ctx) {
            self.pick_layout_file();
        }

        self.file_dialog.update(ctx);

        if let Some(path) = self.file_dialog.take_picked() {
            self.core
                .connection_mgr
                .set_layout_file_path(path.to_string_lossy().to_string());
            self.connect_from_ui();
        } else if self.file_dialog.state() == &egui_file_dialog::DialogState::Cancelled
            && self.core.connection_mgr.state().is_requires_layout_file()
        {
            self.core.connection_mgr.disconnect();
        }

        self.sync_mouse_passthrough(host);
    }

    /// Shows a modal dialog with `message` and an OK button that clears `slot`.
    fn message_window(ctx: &egui::Context, title: &str, slot: &mut Option<String>) {
        let Some(message) = slot.clone() else {
            return;
        };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(message);
                ui.add_space(10.0);
                if ui.button("OK").clicked() {
                    *slot = None;
                }
            });
    }

    /// Renders the overlay in an egui Window.
    pub fn draw_overlay_window(&mut self, ctx: &egui::Context, keyboard: &Keyboard, visible: bool) {
        let anchor_params = self.get_anchor_params();
        let mut window_open = visible;
        let hit_test_enabled = self.is_any_window_open();
        let size = self.core.settings.active.size as f32;

        egui::Window::new("KeyPeek")
            .open(&mut window_open)
            .auto_sized()
            .interactable(hit_test_enabled)
            .anchor(anchor_params.0, anchor_params.1)
            .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
            .fade_out(true)
            .title_bar(false)
            .show(ctx, |ui| {
                self.core
                    .overlay_view(keyboard, size, hit_test_enabled)
                    .show(ui);
            });
    }

    /// Renders desktop windows: overlay, key editor, settings, and notices.
    pub fn render(&mut self, ctx: &egui::Context, host: &mut dyn OverlayHost) {
        let connected = self.core.connection_mgr.connected_pair();
        if let Some((keyboard, profile)) = &connected {
            self.draw_overlay_window(ctx, keyboard, self.overlay_visible());
            self.core
                .draw_editor_window(ctx, keyboard, profile.as_ref());
        }

        if self.ui.settings_visible {
            self.draw_settings_window(ctx, host);
        }

        Self::message_window(ctx, "Error", &mut self.core.ui.settings_error);
        Self::message_window(ctx, "Notice", &mut self.core.ui.settings_warning);
    }

    /// Primary entry point called by the desktop platform runner.
    pub fn ui(&mut self, ctx: &egui::Context, host: &mut dyn OverlayHost) {
        self.update(ctx, host);
        self.render(ctx, host);
        self.schedule_overlay_repaint(ctx);
    }
}
