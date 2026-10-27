//! Pure in-memory keyboard domain aggregate and layout switching state machine.

use std::sync::Mutex;
use std::time::Duration;
use web_time::Instant;

use crate::key_matrix::KeyMatrix;
use crate::key_spec::{KeySpec, LayerInfo};
use crate::layout::{EncoderDirection, KeyboardDefinition, KeyboardLayout};
use crate::layout_key::LayoutKey;
use crate::visibility::{OverlayConfig, VisibilityStateMachine};

/// Pure domain aggregate holding keyboard layout, matrix bindings, layer states,
/// and overlay visibility. Has no background threads or communication channels.
pub struct KeyboardDomain {
    definition: KeyboardDefinition,
    active_layout_name: Mutex<String>,
    layout: Mutex<KeyboardLayout>,
    matrix: Mutex<KeyMatrix>,
    /// `[layer][encoder_id] -> (ccw, cw)`. Populated once at connect time; there is
    /// no live encoder-rotation event to keep it in sync.
    encoder_keys: Vec<Vec<(Option<LayoutKey>, Option<LayoutKey>)>>,
    layer_state: Mutex<u32>,
    default_layer_state: Mutex<u32>,
    visibility: Mutex<VisibilityStateMachine>,
}

impl KeyboardDomain {
    pub fn new(
        definition: KeyboardDefinition,
        active_layout_name: String,
        layout: KeyboardLayout,
        matrix: KeyMatrix,
        encoder_keys: Vec<Vec<(Option<LayoutKey>, Option<LayoutKey>)>>,
        config: OverlayConfig,
    ) -> Self {
        Self {
            definition,
            active_layout_name: Mutex::new(active_layout_name),
            layout: Mutex::new(layout),
            matrix: Mutex::new(matrix),
            encoder_keys,
            layer_state: Mutex::new(0),
            default_layer_state: Mutex::new(0),
            visibility: Mutex::new(VisibilityStateMachine::new(config, Instant::now())),
        }
    }

    // --- State update methods (driven by KeyboardSession) ---

    pub fn on_layers_changed(&self, active_layers: u32, default_layers: u32, now: Instant) -> bool {
        *self.layer_state.lock().unwrap() = active_layers;
        *self.default_layer_state.lock().unwrap() = default_layers;
        self.visibility
            .lock()
            .unwrap()
            .on_layers_changed(active_layers, default_layers, now)
    }

    pub fn on_key_pressed(&self, row: usize, col: usize, pressed: bool, now: Instant) -> bool {
        if let Ok(mut mat) = self.matrix.lock() {
            mat.set_pressed(row, col, pressed);
        }
        self.visibility.lock().unwrap().is_visible(now)
    }

    pub fn layer_info(&self, index: usize) -> Option<LayerInfo> {
        self.matrix
            .lock()
            .unwrap()
            .layer_infos()
            .get(index)
            .cloned()
    }

    pub fn update_binding(
        &self,
        layer: usize,
        row: usize,
        col: usize,
        action: KeySpec,
        label: Option<LayoutKey>,
    ) {
        if let Ok(mut mat) = self.matrix.lock() {
            mat.update_binding(layer, row, col, action, label);
        }
    }

    // --- Domain queries ---

    pub fn overlay_is_visible(&self, now: Instant) -> bool {
        self.visibility.lock().unwrap().is_visible(now)
    }

    /// How long until the overlay appears or disappears on its own, for scheduling a repaint.
    pub fn overlay_changes_in(&self, now: Instant) -> Option<Duration> {
        self.visibility.lock().unwrap().changes_in(now)
    }

    pub fn get_effective_key_layer(&self, row: usize, col: usize) -> (u8, bool) {
        let layer_state = *self.layer_state.lock().unwrap();
        let default_layer_state = *self.default_layer_state.lock().unwrap();
        let matrix = self.matrix.lock().unwrap();
        matrix.effective_layer(layer_state, default_layer_state, row, col)
    }

    pub fn get_key(&self, layer: usize, row: usize, col: usize) -> Option<LayoutKey> {
        self.matrix
            .lock()
            .unwrap()
            .get_key(layer, row, col)
            .cloned()
    }

    /// Mirrors `get_effective_key_layer` for encoders.
    pub fn get_effective_encoder_layer(&self, id: u8) -> (u8, bool) {
        let layer_state = *self.layer_state.lock().unwrap();
        let default_layer_state = *self.default_layer_state.lock().unwrap();
        let num_layers = self.encoder_keys.len().min(32);

        let mut active_layer_above = false;

        for i in (1..num_layers).rev() {
            let layer_mask = 1u32 << (i as u32);
            let is_active_default_layer = (default_layer_state & layer_mask) != 0;
            let is_active_momentary_layer = (layer_state & layer_mask) != 0;
            if (is_active_momentary_layer || is_active_default_layer)
                && !self.is_encoder_transparent(i, id)
            {
                return (i as u8, is_active_default_layer && active_layer_above);
            }
            active_layer_above |= is_active_momentary_layer;
        }

        (0, active_layer_above)
    }

    fn is_encoder_transparent(&self, layer: usize, id: u8) -> bool {
        self.encoder_keys
            .get(layer)
            .and_then(|l| l.get(id as usize))
            .map(|(ccw, cw)| ccw.is_none() && cw.is_none())
            .unwrap_or(true)
    }

    pub fn get_encoder_key(&self, layer: usize, id: u8, clockwise: bool) -> Option<LayoutKey> {
        let (ccw, cw) = self.encoder_keys.get(layer)?.get(id as usize)?;
        (if clockwise { cw } else { ccw }).clone()
    }

    /// `Both` (VIA's one-tile case) synthesizes a combined key reusing the
    /// existing tap+shifted stack display.
    pub fn get_encoder_display_key(
        &self,
        layer: usize,
        id: u8,
        direction: EncoderDirection,
    ) -> LayoutKey {
        match direction {
            EncoderDirection::Clockwise => {
                self.get_encoder_key(layer, id, true).unwrap_or_default()
            }
            EncoderDirection::CounterClockwise => {
                self.get_encoder_key(layer, id, false).unwrap_or_default()
            }
            EncoderDirection::Both => {
                let cw = self.get_encoder_key(layer, id, true).unwrap_or_default();
                let ccw_text = self.get_encoder_key(layer, id, false).map(|k| k.tap.full);
                LayoutKey {
                    shifted: ccw_text,
                    ..cw
                }
            }
        }
    }

    pub fn layer_infos(&self) -> Vec<LayerInfo> {
        self.matrix.lock().unwrap().layer_infos().to_vec()
    }

    pub fn layer_names(&self) -> Vec<String> {
        self.layer_infos()
            .iter()
            .map(|l| l.name.clone().unwrap_or_default())
            .collect()
    }

    pub fn get_action(&self, layer: usize, row: usize, col: usize) -> Option<KeySpec> {
        self.matrix
            .lock()
            .unwrap()
            .get_action(layer, row, col)
            .cloned()
    }

    pub fn is_key_pressed(&self, row: usize, col: usize) -> bool {
        self.matrix.lock().unwrap().is_pressed(row, col)
    }

    pub fn held_mod_mask(&self) -> u16 {
        let guard = self.layout.lock().unwrap();
        let matrix = self.matrix.lock().unwrap();
        let layer_state = *self.layer_state.lock().unwrap();
        let default_layer_state = *self.default_layer_state.lock().unwrap();
        matrix.held_mod_mask(&guard.keys, layer_state, default_layer_state)
    }

    pub fn is_shift_held(&self) -> bool {
        self.held_mod_mask() & crate::layout_key::HELD_MOD_SHIFT != 0
    }

    pub fn is_ralt_held(&self) -> bool {
        self.held_mod_mask() & crate::layout_key::HELD_MOD_RALT != 0
    }

    pub fn set_config(&self, config: OverlayConfig) {
        self.visibility.lock().unwrap().set_config(config);
    }

    pub fn layout_names(&self) -> Vec<String> {
        self.definition.get_layout_names()
    }

    pub fn active_layout_name(&self) -> String {
        self.active_layout_name.lock().unwrap().clone()
    }

    pub fn switch_layout(&self, name: &str) -> Result<(), String> {
        let next_layout = self.definition.get_layout(name)?;
        *self.layout.lock().unwrap() = next_layout;
        *self.active_layout_name.lock().unwrap() = name.to_string();
        Ok(())
    }

    pub fn layout(&self) -> KeyboardLayout {
        self.layout.lock().unwrap().clone()
    }

    pub fn layer_state(&self) -> u32 {
        *self.layer_state.lock().unwrap()
    }

    pub fn default_layer_state(&self) -> u32 {
        *self.default_layer_state.lock().unwrap()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn create_test_domain() -> KeyboardDomain {
        let definition = KeyboardDefinition {
            vid: 0x1234,
            pid: 0x5678,
            rows: 1,
            cols: 2,
            layouts: vec![
                KeyboardLayout {
                    name: "Default".to_string(),
                    encoders: Vec::new(),
                    keys: vec![],
                },
                KeyboardLayout {
                    name: "Alternative".to_string(),
                    encoders: Vec::new(),
                    keys: vec![],
                },
            ],
        };
        let layout = definition.get_layout("Default").unwrap();
        struct DummyPresenter;
        impl crate::key_presenter::KeyPresenter for DummyPresenter {
            fn present_key(&self, _spec: &KeySpec, _layer_names: &[String]) -> Option<LayoutKey> {
                None
            }
        }
        let matrix = KeyMatrix::from_snapshot(
            crate::key_spec::KeymapSnapshot {
                layers: vec![],
                actions: vec![],
            },
            1,
            2,
            &DummyPresenter,
        );
        let config = OverlayConfig {
            timeout_ms: 2000,
            activation_delay_ms: 300,
            visible_layers: u32::MAX,
        };
        KeyboardDomain::new(
            definition,
            "Default".to_string(),
            layout,
            matrix,
            Vec::new(),
            config,
        )
    }

    #[test]
    fn test_layout_names_and_active_layout() {
        let domain = create_test_domain();
        assert_eq!(domain.layout_names(), vec!["Default", "Alternative"]);
        assert_eq!(domain.active_layout_name(), "Default");
    }

    #[test]
    fn test_switch_layout_success_and_failure() {
        let domain = create_test_domain();
        assert!(domain.switch_layout("Alternative").is_ok());
        assert_eq!(domain.active_layout_name(), "Alternative");
        assert_eq!(domain.layout().name, "Alternative");

        let err = domain.switch_layout("NonExistent");
        assert!(err.is_err());
        assert_eq!(domain.active_layout_name(), "Alternative");
    }

    fn domain_with_encoders(
        encoder_keys: Vec<Vec<(Option<LayoutKey>, Option<LayoutKey>)>>,
    ) -> KeyboardDomain {
        let base = create_test_domain();
        KeyboardDomain {
            encoder_keys,
            ..base
        }
    }

    fn bound(keycode: &str) -> Option<LayoutKey> {
        Some(LayoutKey {
            tap: crate::layout_key::Label::new(keycode),
            ..Default::default()
        })
    }

    #[test]
    fn effective_encoder_layer_falls_through_transparent_layers() {
        // Layer 0 bound, layer 1 transparent, layer 2 bound; layer 2 held -> wins outright.
        let domain = domain_with_encoders(vec![
            vec![(bound("KC_VOLD"), bound("KC_VOLU"))],
            vec![(None, None)],
            vec![(bound("KC_LEFT"), bound("KC_RGHT"))],
        ]);
        domain.on_layers_changed(1 << 2, 0, Instant::now());

        assert_eq!(domain.get_effective_encoder_layer(0), (2, false));
        assert_eq!(
            domain.get_encoder_key(2, 0, true).unwrap().tap.full,
            "KC_RGHT"
        );
    }

    #[test]
    fn effective_encoder_layer_falls_back_to_base_when_nothing_held() {
        let domain = domain_with_encoders(vec![
            vec![(bound("KC_VOLD"), bound("KC_VOLU"))],
            vec![(None, None)],
        ]);
        assert_eq!(domain.get_effective_encoder_layer(0), (0, false));
    }
}
