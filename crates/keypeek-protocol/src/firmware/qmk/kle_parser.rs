use keypeek_core::geometry::flattened_top_left_after_center_rotation;
use keypeek_core::{EncoderDirection, EncoderTile, Key, KeyboardDefinition, KeyboardLayout};
use serde_json::Value;
use std::error::Error;

pub fn parse_vial_definition(
    json: &Value,
    vid: u16,
    pid: u16,
) -> Result<KeyboardDefinition, Box<dyn Error>> {
    let keymap = find_kle_keymap(json)
        .ok_or_else(|| Box::<dyn Error>::from("No 'keymap' found in definition"))?;
    parse_kle_definition(json, keymap, vid, pid)
}

use super::catalog::resolve_vendor_product_ids;

pub fn parse_kle_definition(
    json: &Value,
    keymap: &[Value],
    vid: u16,
    pid: u16,
) -> Result<KeyboardDefinition, Box<dyn Error>> {
    let (vid, pid) = resolve_vendor_product_ids(json, vid, pid);
    let matrix = json
        .get("matrix")
        .ok_or_else(|| Box::<dyn Error>::from("No 'matrix' found in definition"))?;

    let rows = matrix
        .get("rows")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Box::<dyn Error>::from("No 'rows' in matrix"))? as usize;

    let cols = matrix
        .get("cols")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Box::<dyn Error>::from("No 'cols' in matrix"))? as usize;

    let (keys, encoders) = parse_kle_keymap(keymap)?;

    let layout = KeyboardLayout {
        name: "default".to_string(),
        encoders,
        keys,
    };

    Ok(KeyboardDefinition {
        vid,
        pid,
        rows,
        cols,
        layouts: vec![layout],
    })
}

/// Locates a KLE raw keymap array in either `layouts.keymap` or top-level `keymap`.
pub fn find_kle_keymap(json: &Value) -> Option<&[Value]> {
    if let Some(layouts) = json.get("layouts") {
        if let Some(keymap) = layouts.get("keymap") {
            if let Some(arr) = keymap.as_array() {
                return Some(arr);
            }
            if let Some(arr) = keymap.get("layout").and_then(Value::as_array) {
                return Some(arr);
            }
        }
    }
    if let Some(keymap) = json.get("keymap") {
        if let Some(arr) = keymap.as_array() {
            return Some(arr);
        }
        if let Some(arr) = keymap.get("layout").and_then(Value::as_array) {
            return Some(arr);
        }
    }
    None
}

struct ViaOption {
    index: usize,
    name: String,
    keys: Vec<Key>,
}

struct ViaOptionGroup {
    index: usize,
    options: Vec<ViaOption>,
}

pub fn parse_via_definition(
    json: &Value,
    vid: u16,
    pid: u16,
) -> Result<KeyboardDefinition, Box<dyn Error>> {
    let (vid, pid) = resolve_vendor_product_ids(json, vid, pid);
    let matrix = json
        .get("matrix")
        .ok_or_else(|| Box::<dyn Error>::from("No 'matrix' found in VIA definition"))?;
    let rows = matrix
        .get("rows")
        .and_then(Value::as_u64)
        .ok_or_else(|| Box::<dyn Error>::from("No 'rows' in matrix"))? as usize;
    let cols = matrix
        .get("cols")
        .and_then(Value::as_u64)
        .ok_or_else(|| Box::<dyn Error>::from("No 'cols' in matrix"))? as usize;
    let layouts = json
        .get("layouts")
        .ok_or_else(|| Box::<dyn Error>::from("No 'layouts' in VIA definition"))?;
    let raw_keys = layouts
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| Box::<dyn Error>::from("No 'keys' in VIA layouts"))?;

    let base_keys = parse_via_keys(raw_keys)?;
    let option_groups = parse_via_option_groups(layouts, layouts.get("labels"))?;
    let layouts = build_via_layouts(base_keys, &option_groups);

    Ok(KeyboardDefinition {
        vid,
        pid,
        rows,
        cols,
        layouts,
    })
}

fn parse_via_option_groups(
    layouts: &Value,
    labels: Option<&Value>,
) -> Result<Vec<ViaOptionGroup>, Box<dyn Error>> {
    let mut option_groups = Vec::new();
    let Some(groups) = layouts.get("optionKeys").and_then(Value::as_object) else {
        return Ok(option_groups);
    };

    for (group_str, options_val) in groups {
        let Some(options_obj) = options_val.as_object() else {
            continue;
        };
        let group_idx = group_str.parse::<usize>().unwrap_or(usize::MAX);
        let mut parsed_options = Vec::new();

        for (option_str, keys_val) in options_obj {
            let Some(keys_arr) = keys_val.as_array() else {
                continue;
            };
            let option_idx = option_str.parse::<usize>().unwrap_or(usize::MAX);
            let keys = parse_via_keys(keys_arr)?;
            let opt_name = resolve_via_option_name(labels, group_idx, option_idx)
                .unwrap_or_else(|| format!("{group_str}={option_str}"));
            parsed_options.push(ViaOption {
                index: option_idx,
                name: opt_name,
                keys,
            });
        }

        parsed_options.sort_by_key(|opt| opt.index);
        if !parsed_options.is_empty() {
            option_groups.push(ViaOptionGroup {
                index: group_idx,
                options: parsed_options,
            });
        }
    }

    option_groups.sort_by_key(|g| g.index);
    Ok(option_groups)
}

fn build_via_layouts(base_keys: Vec<Key>, option_groups: &[ViaOptionGroup]) -> Vec<KeyboardLayout> {
    if option_groups.is_empty() {
        return vec![KeyboardLayout {
            name: "default".to_string(),
            encoders: Vec::new(),
            keys: base_keys,
        }];
    }

    let mut layouts = Vec::new();

    // 1. Always create the Default layout (option 0 for all groups)
    let mut default_keys = base_keys.clone();
    for group in option_groups {
        if let Some(first_opt) = group.options.first() {
            default_keys.extend(first_opt.keys.clone());
        }
    }
    layouts.push(KeyboardLayout {
        name: "default".to_string(),
        encoders: Vec::new(),
        keys: default_keys,
    });

    // 2. Generate combinations or individual option variants
    let total_combinations: usize = option_groups
        .iter()
        .map(|g| g.options.len())
        .fold(1usize, |acc, count| acc.saturating_mul(count));

    if total_combinations > 1 && total_combinations <= 32 {
        let mut combinations: Vec<Vec<(String, Vec<Key>, bool)>> = vec![Vec::new()];
        for group in option_groups {
            let mut next = Vec::new();
            for comb in &combinations {
                for opt in &group.options {
                    let mut next_comb = comb.clone();
                    let is_default = opt.index == 0;
                    next_comb.push((opt.name.clone(), opt.keys.clone(), is_default));
                    next.push(next_comb);
                }
            }
            combinations = next;
        }

        for combination in combinations {
            let all_default = combination.iter().all(|(_, _, is_def)| *is_def);
            if all_default {
                continue; // Already added as "default"
            }

            let non_default_names: Vec<String> = combination
                .iter()
                .filter(|(_, _, is_def)| !*is_def)
                .map(|(name, _, _)| name.clone())
                .collect();

            let mut keys = base_keys.clone();
            for (_, opt_keys, _) in combination {
                keys.extend(opt_keys);
            }

            layouts.push(KeyboardLayout {
                name: format!("VIA ({})", non_default_names.join(", ")),
                encoders: Vec::new(),
                keys,
            });
        }
    } else if total_combinations > 32 {
        // High permutation count: add one layout variant per non-default option
        for target_group in option_groups {
            for opt in &target_group.options {
                if opt.index == 0 {
                    continue;
                }
                let mut keys = base_keys.clone();
                for group in option_groups {
                    if group.index == target_group.index {
                        keys.extend(opt.keys.clone());
                    } else if let Some(def_opt) = group.options.first() {
                        keys.extend(def_opt.keys.clone());
                    }
                }
                layouts.push(KeyboardLayout {
                    name: format!("VIA ({})", opt.name),
                    encoders: Vec::new(),
                    keys,
                });
            }
        }
    }

    layouts
}

fn resolve_via_option_name(
    labels: Option<&Value>,
    group_idx: usize,
    option_idx: usize,
) -> Option<String> {
    let group_val = labels.and_then(|l| l.get(group_idx))?;
    if let Some(s) = group_val.as_str() {
        if option_idx == 0 {
            Some(format!("Default {s}"))
        } else {
            Some(s.to_string())
        }
    } else if let Some(arr) = group_val.as_array() {
        let opt_label = arr.get(option_idx + 1).and_then(Value::as_str)?;
        if let Some(group_title) = arr.first().and_then(Value::as_str) {
            Some(format!("{group_title}: {opt_label}"))
        } else {
            Some(opt_label.to_string())
        }
    } else {
        None
    }
}

fn parse_via_keys(raw_keys: &[Value]) -> Result<Vec<Key>, Box<dyn Error>> {
    let mut keys = Vec::with_capacity(raw_keys.len());
    for raw_key in raw_keys {
        let key = raw_key
            .as_object()
            .ok_or_else(|| Box::<dyn Error>::from("VIA layout key is not an object"))?;

        // Skip decal / cosmetic elements
        if key.get("d").and_then(Value::as_bool).unwrap_or(false) {
            continue;
        }

        // Keys without row or col are decals, plate borders, or spacers
        let (Some(row), Some(col)) = (
            key.get("row").and_then(Value::as_u64),
            key.get("col").and_then(Value::as_u64),
        ) else {
            continue;
        };

        let x = key
            .get("x")
            .and_then(Value::as_f64)
            .ok_or_else(|| Box::<dyn Error>::from("VIA layout key has no x"))?
            as f32;
        let y = key
            .get("y")
            .and_then(Value::as_f64)
            .ok_or_else(|| Box::<dyn Error>::from("VIA layout key has no y"))?
            as f32;
        let w = key.get("w").and_then(Value::as_f64).unwrap_or(1.0) as f32;
        let h = key.get("h").and_then(Value::as_f64).unwrap_or(1.0) as f32;
        let r = key.get("r").and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let rx = key
            .get("rx")
            .and_then(Value::as_f64)
            .map(|v| v as f32)
            .unwrap_or(x);
        let ry = key
            .get("ry")
            .and_then(Value::as_f64)
            .map(|v| v as f32)
            .unwrap_or(y);

        let (final_x, final_y) = flattened_top_left_after_center_rotation(x, y, w, h, r, rx, ry);

        keys.push(Key {
            row: row as usize,
            col: col as usize,
            x: final_x,
            y: final_y,
            w,
            h,
            r,
        });
    }
    Ok(keys)
}

struct RawEncoderTile {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
}

fn parse_kle_keymap(keymap: &[Value]) -> Result<(Vec<Key>, Vec<EncoderTile>), Box<dyn Error>> {
    let mut keys = Vec::new();
    let mut raw_encoders = Vec::new();
    let mut current_y: f32 = 0.0;
    let mut rotation_angle: f32 = 0.0;
    let mut rotation_x: f32 = 0.0;
    let mut rotation_y: f32 = 0.0;

    for row in keymap {
        let row_array = match row.as_array() {
            Some(arr) => arr,
            None => continue, // Skip non-array rows
        };

        let mut current_x: f32 = 0.0;
        let mut current_w: f32 = 1.0;
        let mut current_h: f32 = 1.0;

        for item in row_array {
            if let Some(obj) = item.as_object() {
                // Handle rotation properties (these persist until changed)
                // When rx or ry changes, reset the current position to the rotation origin
                let mut origin_changed = false;
                if let Some(rx) = obj.get("rx").and_then(|v| v.as_f64()) {
                    rotation_x = rx as f32;
                    origin_changed = true;
                }
                if let Some(ry) = obj.get("ry").and_then(|v| v.as_f64()) {
                    rotation_y = ry as f32;
                    origin_changed = true;
                }

                if origin_changed {
                    current_x = 0.0;
                    current_y = 0.0;
                }

                if let Some(r) = obj.get("r").and_then(|v| v.as_f64()) {
                    rotation_angle = r as f32;
                }

                if let Some(w) = obj.get("w").and_then(|v| v.as_f64()) {
                    current_w = w as f32;
                }
                if let Some(h) = obj.get("h").and_then(|v| v.as_f64()) {
                    current_h = h as f32;
                }
                if let Some(x) = obj.get("x").and_then(|v| v.as_f64()) {
                    current_x += x as f32;
                }
                if let Some(y) = obj.get("y").and_then(|v| v.as_f64()) {
                    current_y += y as f32;
                }
            } else if let Some(label) = item.as_str() {
                // Normalize KLE-relative coordinates to absolute space, then flatten rotation.
                let absolute_x = rotation_x + current_x;
                let absolute_y = rotation_y + current_y;
                let (final_x, final_y) = flattened_top_left_after_center_rotation(
                    absolute_x,
                    absolute_y,
                    current_w,
                    current_h,
                    rotation_angle,
                    rotation_x,
                    rotation_y,
                );

                if is_encoder_label(label) {
                    raw_encoders.push(RawEncoderTile {
                        x: final_x,
                        y: final_y,
                        w: current_w,
                        h: current_h,
                        r: rotation_angle,
                    });
                } else if let Some((row, col)) = parse_matrix_label(label) {
                    keys.push(Key {
                        row,
                        col,
                        x: final_x,
                        y: final_y,
                        w: current_w,
                        h: current_h,
                        r: rotation_angle,
                    });
                }

                current_x += current_w;

                current_w = 1.0;
                current_h = 1.0;
            }
        }

        current_y += 1.0;
    }

    let encoders = group_encoder_tiles(raw_encoders);

    Ok((keys, encoders))
}

/// Vial draws one encoder as two stacked 1u cells.
fn group_encoder_tiles(raw: Vec<RawEncoderTile>) -> Vec<EncoderTile> {
    let mut column_xs: Vec<f32> = raw.iter().map(|t| t.x).collect();
    column_xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    column_xs.dedup_by(|a, b| (*a - *b).abs() < 0.01);

    raw.iter()
        .map(|t| {
            let id = column_xs
                .iter()
                .position(|&x| (x - t.x).abs() < 0.01)
                .unwrap_or(0) as u8;
            let is_topmost = raw.iter().all(|o| (o.x - t.x).abs() >= 0.01 || o.y >= t.y);
            EncoderTile {
                id,
                direction: if is_topmost {
                    EncoderDirection::Clockwise
                } else {
                    EncoderDirection::CounterClockwise
                },
                x: t.x,
                y: t.y,
                w: t.w,
                h: t.h,
                r: t.r,
            }
        })
        .collect()
}

/// Vial's KLE convention for a rotation-only cell: label's last line is "e".
fn is_encoder_label(label: &str) -> bool {
    label.lines().last() == Some("e")
}

fn parse_matrix_label(label: &str) -> Option<(usize, usize)> {
    let first_line = label.lines().next().unwrap_or(label);

    let parts: Vec<&str> = first_line.split(',').collect();
    if parts.len() >= 2 {
        let row = parts[0].trim().parse::<usize>().ok()?;
        let col = parts[1].trim().parse::<usize>().ok()?;
        return Some((row, col));
    }

    None
}

#[cfg(test)]
mod encoder_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn is_encoder_label_matches_vials_e_convention() {
        assert!(is_encoder_label("0,0\ne"));
        assert!(is_encoder_label("e"));
        assert!(!is_encoder_label("0,0"));
        assert!(!is_encoder_label(""));
    }

    #[test]
    fn stacked_encoder_cells_share_an_id_topmost_is_clockwise() {
        let keymap = json!([["e"], ["e"], ["0,0"],]);
        let keymap = keymap.as_array().unwrap();
        let (keys, encoders) = parse_kle_keymap(keymap).unwrap();

        assert_eq!(keys.len(), 1, "the one real matrix key must still parse");
        assert_eq!(encoders.len(), 2);
        assert_eq!(
            encoders[0].id, encoders[1].id,
            "both tiles share one encoder id"
        );

        let clockwise_tile = encoders
            .iter()
            .find(|e| e.direction == EncoderDirection::Clockwise)
            .unwrap();
        let other = encoders
            .iter()
            .find(|e| e.direction == EncoderDirection::CounterClockwise)
            .unwrap();
        assert!(clockwise_tile.y < other.y);
    }
}
