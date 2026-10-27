//! Shared application core state, connection lifecycle, and editor synchronization.

use std::sync::Arc;
use web_time::Instant;

use super::state::{SettingsState, UiState};
use super::ui_overlay::OverlayView;
use crate::key_paint::KeyPaintStyle;
use crate::keymap_editor::{EditorProfile, EditorState};
use crate::settings::SettingsStore;
use keypeek_protocol::connection_manager::{
    ConnectOutcome, ConnectionEvent, DeviceConnectionManager,
};
use keypeek_protocol::{DiscoveredDevice, Keyboard, UiWake};

/// Core state and business logic shared across presentation targets (Desktop and Web).
///
/// Encapsulates:
/// - UI alert state and visibility
/// - Active and draft settings
/// - Connection manager lifecycle and event dispatching
/// - Keymap editor lifecycle
pub struct KeyPeekAppCore {
    pub ui: UiState,
    pub settings: SettingsState,
    pub settings_store: Arc<dyn SettingsStore>,
    pub connection_mgr: DeviceConnectionManager,
    pub editor: EditorState,
}

impl KeyPeekAppCore {
    /// Creates a new `KeyPeekAppCore` with loaded settings and an initialized connection manager.
    pub fn new(
        ui_wake: UiWake,
        settings_store: Arc<dyn SettingsStore>,
        available_devices: Vec<DiscoveredDevice>,
        settings_visible: bool,
    ) -> Self {
        let base_settings = settings_store.load();

        Self {
            ui: UiState {
                settings_visible,
                settings_error: None,
                settings_warning: None,
            },
            settings: SettingsState::new(base_settings),
            settings_store,
            connection_mgr: DeviceConnectionManager::new(available_devices, ui_wake),
            editor: EditorState::new(),
        }
    }

    /// Whether any modal or editor window/sidebar is open.
    pub fn is_any_window_open(&self) -> bool {
        self.ui.settings_visible || self.editor.is_open()
    }

    /// Persists the active settings to the configured settings store.
    pub fn persist_settings(&self) {
        if let Err(e) = self.settings_store.save(&self.settings.active) {
            eprintln!("Failed to save settings: {e}");
        }
    }

    /// Commits modified draft settings to active settings, updates connected keyboard config,
    /// and persists to the settings store. Returns `true` if changes were committed.
    pub fn sync_visual_settings(&mut self) -> bool {
        if self.settings.commit_draft() {
            if let Some(keyboard) = self.connection_mgr.connected_keyboard() {
                keyboard.set_config(self.settings.overlay_config());
            }
            self.persist_settings();
            true
        } else {
            false
        }
    }

    /// Closes the editor window/sidebar and releases any device write lock.
    pub fn close_editor(&mut self) {
        self.editor
            .close(self.connection_mgr.connected_keyboard().as_deref());
    }

    /// Requests the editor window to close, initiating a ZMK save first if changes
    /// are pending; otherwise closes immediately.
    pub fn request_close_editor(&mut self) -> bool {
        self.editor
            .handle_close_request(self.connection_mgr.connected_keyboard().as_deref())
    }

    /// Switches the active keyboard layout and saves the preferred layout name.
    pub fn switch_layout(&mut self, name: &str) {
        if let Some(keyboard) = self.connection_mgr.connected_keyboard() {
            if let Err(e) = keyboard.switch_layout(name) {
                self.ui.set_error(e);
            } else {
                self.connection_mgr
                    .set_preferred_layout_name(Some(name.to_string()));
            }
        }
    }

    /// Disconnects the active keyboard connection, tears down protocol session,
    /// and closes the keymap editor.
    pub fn disconnect_from_ui(&mut self) {
        self.close_editor();
        self.connection_mgr.disconnect();
        self.ui.clear_alerts();
    }

    /// Initiates connection for the currently selected device in connection_mgr.
    pub fn connect_selected_device(&mut self) -> ConnectOutcome {
        let outcome = self.connection_mgr.connect(self.settings.overlay_config());
        match &outcome {
            ConnectOutcome::Started => {
                self.ui.settings_error = None;
            }
            ConnectOutcome::AlreadyConnected => {
                self.ui
                    .set_warning("Already connected. Disconnect first to switch devices.");
            }
            ConnectOutcome::Failed(e) => {
                self.ui.set_error(e.clone());
            }
            ConnectOutcome::RequiresLayoutFile => {}
        }
        outcome
    }

    /// Checks if the keyboard overlay should be visible according to open windows
    /// or active layer hold timeouts.
    pub fn is_overlay_visible(&self) -> bool {
        if let Some(keyboard) = self.connection_mgr.connected_keyboard() {
            self.is_any_window_open() || keyboard.overlay_is_visible(Instant::now())
        } else {
            false
        }
    }

    /// Schedules a repaint when the overlay visibility timer expires.
    pub fn schedule_overlay_repaint(&self, ctx: &egui::Context) {
        if self.is_any_window_open() {
            return;
        }

        let Some(keyboard) = self.connection_mgr.connected_keyboard() else {
            return;
        };

        if let Some(delay) = keyboard.overlay_changes_in(Instant::now()) {
            ctx.request_repaint_after(delay);
        }
    }

    /// Polls the connected keyboard, advances the connection state machine,
    /// and dispatches standard lifecycle events (`Connected`, `ConnectionFailed`,
    /// `Disconnected`, `DeviceLocked`).
    ///
    /// Returns any emitted [`ConnectionEvent`] for platform-specific handling
    /// (e.g. file picking or clearing platform state).
    pub fn update_connection(&mut self, ctx: &egui::Context) -> Option<ConnectionEvent> {
        if let Some(keyboard) = self.connection_mgr.connected_keyboard() {
            keyboard.poll();
        }

        let event = self
            .connection_mgr
            .update(self.settings.overlay_config(), |d| {
                ctx.request_repaint_after(d);
            });

        if let Some(event) = &event {
            match event {
                ConnectionEvent::Connected => {
                    self.ui.clear_alerts();
                    self.sync_visual_settings();
                }
                ConnectionEvent::ConnectionFailed(e) => {
                    self.ui.set_error(e.clone());
                }
                ConnectionEvent::Disconnected => {
                    self.close_editor();
                }
                ConnectionEvent::DeviceLocked(dev) => {
                    self.ui.set_error(dev.lock_message());
                }
                ConnectionEvent::RequiresLayoutFile(_) => {}
            }
        }

        if !self.connection_mgr.is_connected() && self.editor.is_open() {
            self.close_editor();
        }

        event
    }

    /// Creates an `OverlayView` component with active settings and the keymap editor.
    pub fn overlay_view<'a>(
        &'a mut self,
        keyboard: &'a Keyboard,
        size: f32,
        hit_test_enabled: bool,
    ) -> OverlayView<'a> {
        OverlayView::new(
            keyboard,
            &mut self.editor,
            &self.settings.active,
            size,
            hit_test_enabled,
        )
    }

    /// Creates a `KeyPaintStyle` for the specified unit size from active settings.
    pub fn paint_style(&self, unit: f32) -> KeyPaintStyle {
        self.settings.paint_style(unit)
    }

    /// Draws the keymap editor in a floating window (desktop).
    pub fn draw_editor_window(
        &mut self,
        ctx: &egui::Context,
        keyboard: &Keyboard,
        profile: &dyn EditorProfile,
    ) {
        if self.editor.is_open() {
            let style = self.paint_style(crate::keymap_editor::KEY_UNIT);
            self.editor.draw_window(ctx, keyboard, profile, &style);
        }
    }

    /// Draws the keymap editor in a sidebar panel (web).
    pub fn draw_editor_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        keyboard: &Keyboard,
        profile: &dyn EditorProfile,
    ) {
        if self.editor.is_open() {
            let style = self.paint_style(crate::keymap_editor::KEY_UNIT);
            self.editor.draw_sidebar(ui, keyboard, profile, &style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap_editor::EditTarget;
    use crate::settings::MemorySettingsStore;

    fn make_test_core(settings_visible: bool) -> KeyPeekAppCore {
        let ui_wake = UiWake::new(Arc::new(|| {}));
        let store = Arc::new(MemorySettingsStore::default());
        KeyPeekAppCore::new(ui_wake, store, Vec::new(), settings_visible)
    }

    #[test]
    fn test_app_core_initialization() {
        let core = make_test_core(false);
        assert!(!core.is_any_window_open());
        assert!(!core.ui.settings_visible);
        assert!(core.connection_mgr.available_devices().is_empty());
    }

    #[test]
    fn test_is_any_window_open() {
        let mut core = make_test_core(false);
        assert!(!core.is_any_window_open());

        core.ui.settings_visible = true;
        assert!(core.is_any_window_open());

        core.ui.settings_visible = false;
        assert!(!core.is_any_window_open());

        core.editor.target = Some(EditTarget::new(0, 0, 0));
        assert!(core.is_any_window_open());
    }

    #[test]
    fn test_sync_visual_settings_persists() {
        let mut core = make_test_core(false);
        assert!(!core.sync_visual_settings());

        core.settings.draft.timeout = 7777;
        assert!(core.sync_visual_settings());
        assert_eq!(core.settings.active.timeout, 7777);

        let reloaded = core.settings_store.load();
        assert_eq!(reloaded.timeout, 7777);

        assert!(!core.sync_visual_settings());
    }

    #[test]
    fn test_disconnect_from_ui_clears_alerts() {
        let mut core = make_test_core(false);
        core.ui.set_error("Connection timed out");
        assert!(core.ui.settings_error.is_some());

        core.disconnect_from_ui();
        assert!(core.ui.settings_error.is_none());
        assert!(!core.editor.is_open());
    }

    #[test]
    fn test_connect_selected_device_without_selection() {
        let mut core = make_test_core(false);
        let outcome = core.connect_selected_device();
        assert!(matches!(outcome, ConnectOutcome::Failed(_)));
        assert!(core.ui.settings_error.is_some());
    }
}
