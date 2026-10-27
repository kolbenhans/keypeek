use keypeek_core::geometry::flattened_top_left_after_center_rotation;
use keypeek_core::{EncoderDirection, EncoderTile, Key, KeyboardDefinition, KeyboardLayout};
use serde_json::Value;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::num::ParseIntError;

pub fn parse_qmk_json(json_path: &str) -> Result<KeyboardDefinition, Box<dyn Error>> {
    let file = File::open(json_path).map_err(|e| {
        Box::<dyn Error>::from(format!(
            "Failed to open keyboard info JSON '{}': {}",
            json_path, e
        ))
    })?;
    let reader = BufReader::new(file);
    let json: Value = serde_json::from_reader(reader).map_err(|e| {
        Box::<dyn Error>::from(format!("Failed to parse JSON '{}': {}", json_path, e))
    })?;

    parse_qmk_json_value(&json)
}

pub fn parse_qmk_json_value(json: &Value) -> Result<KeyboardDefinition, Box<dyn Error>> {
    let mut layouts = Vec::new();
    let raw_layouts = json["layouts"]
        .as_object()
        .ok_or_else(|| Box::<dyn Error>::from("No layouts found in keyboard info JSON."))?;

    for layout_name in raw_layouts.keys() {
        let raw_layout = &raw_layouts[layout_name];
        let (keys, encoders) = collect_layout_keys(raw_layout)?;
        let layout = KeyboardLayout {
            name: layout_name.clone(),
            encoders,
            keys,
        };
        layouts.push(layout);
    }

    let is_split_keyboard = json
        .get("split")
        .unwrap_or(&Value::Null)
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let row_multiplier = if is_split_keyboard { 2 } else { 1 };

    let matrix_pins = json.get("matrix_pins").ok_or_else(|| {
        Box::<dyn Error>::from("Unable to find 'matrix_pins' in keyboard info JSON.")
    })?;

    let rows = matrix_pins
        .get("rows")
        .ok_or_else(|| Box::<dyn Error>::from("Unable to find 'rows' in 'matrix_pins'."))?
        .as_array()
        .ok_or_else(|| Box::<dyn Error>::from("Rows in matrix_pins is not an array."))?
        .len()
        * row_multiplier;

    let cols = matrix_pins
        .get("cols")
        .ok_or_else(|| Box::<dyn Error>::from("Unable to find 'cols' in 'matrix_pins'."))?
        .as_array()
        .ok_or_else(|| Box::<dyn Error>::from("Cols in matrix_pins is not an array."))?
        .len();

    let usb = json
        .get("usb")
        .ok_or_else(|| Box::<dyn Error>::from("Unable to find 'usb' in keyboard info JSON."))?;

    let vid_str = usb
        .get("vid")
        .ok_or_else(|| Box::<dyn Error>::from("Unable to find 'vid' in 'usb'."))?
        .as_str()
        .ok_or_else(|| Box::<dyn Error>::from("Unable to convert 'vid' to string."))?;
    let vid = hex_to_u16(vid_str)
        .map_err(|e| Box::<dyn Error>::from(format!("Invalid value for 'vid': {}", e)))?;

    let pid_str = usb
        .get("pid")
        .ok_or_else(|| Box::<dyn Error>::from("Unable to find 'pid' in 'usb'."))?
        .as_str()
        .ok_or_else(|| Box::<dyn Error>::from("Unable to convert 'pid' to string."))?;
    let pid = hex_to_u16(pid_str)
        .map_err(|e| Box::<dyn Error>::from(format!("Invalid value for 'pid': {}", e)))?;

    Ok(KeyboardDefinition {
        vid,
        pid,
        rows,
        cols,
        layouts,
    })
}

pub fn collect_layout_keys(layout: &Value) -> Result<(Vec<Key>, Vec<EncoderTile>), Box<dyn Error>> {
    let layout = layout["layout"]
        .as_array()
        .ok_or_else(|| Box::<dyn Error>::from("No layout array found."))?;

    let mut keys = Vec::new();
    let mut encoders = Vec::new();
    for key in layout {
        // Encoder-only entries have no matrix position.
        let matrix: Option<Vec<usize>> = key["matrix"].as_array().map(|matrix_values| {
            matrix_values
                .iter()
                .filter_map(|v| v.as_u64())
                .map(|n| n as usize)
                .collect()
        });

        let x = key["x"].as_f64().unwrap_or(0.0) as f32;
        let y = key["y"].as_f64().unwrap_or(0.0) as f32;
        let w = key["w"].as_f64().unwrap_or(1.0) as f32;
        let h = key["h"].as_f64().unwrap_or(1.0) as f32;

        // Position is where the key's center lands after rotating around the pivot;
        // the rotation itself is applied at render time via `r`.
        let angle_deg = key.get("r").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let pivot_x = key
            .get("rx")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(x);
        let pivot_y = key
            .get("ry")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(y);

        let (x, y) =
            flattened_top_left_after_center_rotation(x, y, w, h, angle_deg, pivot_x, pivot_y);

        if let Some(matrix) = &matrix {
            if matrix.len() >= 2 {
                keys.push(Key {
                    row: matrix[0],
                    col: matrix[1],
                    x,
                    y,
                    w,
                    h,
                    r: angle_deg,
                });
            }
        }

        // QMK's info.json links a layout entry to an encoder via `"encoder": N`.
        if let Some(id) = key.get("encoder").and_then(|v| v.as_u64()) {
            encoders.push(EncoderTile {
                id: id as u8,
                direction: EncoderDirection::Both,
                x,
                y,
                w,
                h,
                r: angle_deg,
            });
        }
    }

    Ok((keys, encoders))
}

fn hex_to_u16(hex_string: &str) -> Result<u16, ParseIntError> {
    let cleaned_hex = hex_string.trim_start_matches("0x");
    u16::from_str_radix(cleaned_hex, 16)
}

/// Parses a layout JSON string, supporting both VIA/Vial KLE format and QMK info.json format.
pub fn parse_layout_json_str(
    content: &str,
    vid: u16,
    pid: u16,
) -> Result<KeyboardDefinition, crate::protocols::DeviceError> {
    use crate::protocols::DeviceError;

    let json: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| DeviceError::Protocol(format!("Invalid layout JSON: {e}")))?;

    // 1. VIA / Vial definition format: has "matrix" and either "layouts.keys" or a KLE keymap
    if let Some(_matrix) = json.get("matrix") {
        if let Some(layouts) = json.get("layouts") {
            if layouts.get("keys").and_then(Value::as_array).is_some() {
                return super::kle_parser::parse_via_definition(&json, vid, pid).map_err(|e| {
                    DeviceError::Protocol(format!("Failed to parse VIA definition: {e}"))
                });
            }
        }
        if let Some(keymap) = super::kle_parser::find_kle_keymap(&json) {
            return super::kle_parser::parse_kle_definition(&json, keymap, vid, pid).map_err(|e| {
                DeviceError::Protocol(format!("Failed to parse VIA/Vial definition: {e}"))
            });
        }
    }

    // 2. QMK info.json format: has "layouts"
    if let Some(layouts_obj) = json.get("layouts").and_then(|v| v.as_object()) {
        if let Ok(def) = parse_qmk_json_value(&json) {
            return Ok(def);
        }

        // Fallback: parse layouts without requiring matrix_pins or usb in JSON
        let mut parsed_layouts = Vec::new();
        let mut max_row = 0;
        let mut max_col = 0;

        for (layout_name, raw_layout) in layouts_obj {
            if let Ok((keys, encoders)) = collect_layout_keys(raw_layout) {
                for key in &keys {
                    max_row = max_row.max(key.row);
                    max_col = max_col.max(key.col);
                }
                if !keys.is_empty() {
                    parsed_layouts.push(KeyboardLayout {
                        name: layout_name.clone(),
                        encoders,
                        keys,
                    });
                }
            }
        }

        if !parsed_layouts.is_empty() {
            let rows = json
                .get("matrix")
                .and_then(|m| m.get("rows"))
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(max_row + 1);
            let cols = json
                .get("matrix")
                .and_then(|m| m.get("cols"))
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(max_col + 1);

            return Ok(KeyboardDefinition {
                vid,
                pid,
                rows,
                cols,
                layouts: parsed_layouts,
            });
        }
    }

    Err(DeviceError::Protocol(
        "Unrecognized layout format. Expected a VIA layout JSON or QMK info.json.".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::parse_layout_json_str;

    #[test]
    fn parses_official_via_generated_definition() {
        let content = r#"
        {
            "vendorProductId": 1077979750,
            "matrix": {"rows": 5, "cols": 15},
            "layouts": {
                "labels": ["Split Backspace"],
                "width": 16,
                "height": 5,
                "keys": [
                    {"row": 0, "col": 0, "x": 0, "y": 0, "r": 0, "w": 1, "h": 1},
                    {"row": 4, "col": 14, "x": 15, "y": 4, "r": 15, "w": 1.5, "h": 1},
                    {"d": true, "x": 0, "y": 0, "w": 1, "h": 1},
                    {"x": 1, "y": 1, "w": 1, "h": 1}
                ],
                "optionKeys": {
                    "0": {
                        "0": [{"row": 1, "col": 0, "x": 0, "y": 1}],
                        "1": [
                            {"row": 1, "col": 0, "x": 0, "y": 1},
                            {"row": 1, "col": 1, "x": 1, "y": 1}
                        ]
                    }
                }
            }
        }
        "#;

        let definition = parse_layout_json_str(content, 0x1234, 0x5678).unwrap();
        assert_eq!(definition.rows, 5);
        assert_eq!(definition.cols, 15);
        // Base keys has 2 physical switches (the decal and the key without row were skipped)
        // Default layout has base keys (2) + option 0 (1) = 3
        assert_eq!(definition.layouts[0].name, "default");
        assert_eq!(definition.layouts[0].keys.len(), 3);
        assert_eq!(definition.layouts[0].keys[1].row, 4);
        assert_eq!(definition.layouts[0].keys[1].col, 14);
        assert_eq!(definition.layouts[0].keys[1].r, 15.0);

        // Variant layout has base keys (2) + option 1 (2) = 4
        assert_eq!(definition.layouts.len(), 2);
        assert_eq!(definition.layouts[1].name, "VIA (Split Backspace)");
        assert_eq!(definition.layouts[1].keys.len(), 4);
    }
}
