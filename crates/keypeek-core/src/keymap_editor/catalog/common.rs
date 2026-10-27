//! Common candidate key builders for keymap editor profiles.
//!
//! Provides builders for key candidate groups represented as [`KeySpec`] objects.

use crate::key_presenter::KeyPresenter;
use crate::key_spec::Modifiers;
use crate::key_spec::{
    AudioAction, BacklightAction, BluetoothAction, CustomBinding, CustomKind, HidKey, KeySpec,
    LayerActivation, LightingAction, MouseAction, MouseButton, OutputTarget, PowerAction,
    RgbAction, RgbMatrixAction,
};
use crate::keymap_editor::candidate::{Candidate, CandidateGroup, ParameterSpec};

/// Creates a candidate from a [`KeySpec`] and attaches search tokens.
pub fn action_candidate(spec: KeySpec, names: &[&str], presenter: &dyn KeyPresenter) -> Candidate {
    Candidate::from_action(spec, presenter, &[]).with_search_tokens(names.iter().copied())
}

/// Attaches common search aliases to keyboard usages.
pub fn attach_friendly_keyboard_aliases(cand: Candidate, id: u16) -> Candidate {
    let aliases: &[&str] = match id {
        0x28 => &["enter", "return"],
        0x29 => &["esc", "escape"],
        0x2A => &["backspace", "bspc"],
        0x2B => &["tab"],
        0x2C => &["space", "spc"],
        0x2D => &["minus"],
        0x2E => &["equal"],
        0x2F | 0x30 => &["bracket"],
        0x31 => &["backslash"],
        0x33 => &["semicolon"],
        0x34 => &["quote"],
        0x35 => &["grave", "tilde"],
        0x36 => &["comma"],
        0x37 => &["dot", "period"],
        0x38 => &["slash"],
        0x39 => &["caps", "capslock"],
        0x46 => &["print", "printscreen", "prtsc"],
        0x47 => &["scroll", "scrolllock"],
        0x48 => &["pause", "break"],
        0x49 => &["insert", "ins"],
        0x4A => &["home"],
        0x4B => &["pageup", "pgup"],
        0x4C => &["delete", "del"],
        0x4D => &["end"],
        0x4E => &["pagedown", "pgdn"],
        0x4F => &["right"],
        0x50 => &["left"],
        0x51 => &["down"],
        0x52 => &["up"],
        0x53..=0x67 => &["keypad"],
        0xE0 | 0xE4 => &["ctrl", "control"],
        0xE1 | 0xE5 => &["shift"],
        0xE2 | 0xE6 => &["alt"],
        0xE3 | 0xE7 => &["gui", "win", "cmd", "super"],
        _ => &[],
    };
    cand.with_search_tokens(aliases.iter().copied())
}

/// Attaches standard friendly aliases (e.g. "mute", "volup", "play") to consumer usages.
pub fn attach_friendly_media_aliases(cand: Candidate, id: u16) -> Candidate {
    let aliases: &[&str] = match id {
        0xE2 => &["mute", "audio"],
        0xE9 => &["volume", "volup"],
        0xEA => &["volume", "voldn"],
        0xCD => &["play", "pause"],
        0xB5 => &["next", "track"],
        0xB6 => &["prev", "previous", "track"],
        0xB7 => &["stop"],
        0xB8 => &["eject"],
        0x192 => &["calc", "calculator"],
        0x18A => &["mail", "email"],
        0x221 => &["search", "browser"],
        0x223 => &["home", "browser"],
        0x224 => &["back", "browser"],
        0x225 => &["forward", "browser"],
        0x226 => &["stop", "browser"],
        0x227 => &["refresh", "browser"],
        0x6F => &["brightness", "briup"],
        0x70 => &["brightness", "bridn"],
        _ => &[],
    };
    cand.with_search_tokens(aliases.iter().copied())
}

/// Returns `true` if a USB HID Keyboard Page (0x07) usage ID belongs in the standard "Basic" group
/// (standard typing keys, layout/whitespace keys, navigation cluster, arrows, standard numpad, F1-F12).
pub fn is_basic_keyboard_usage(usage_id: u16) -> bool {
    matches!(
        usage_id,
        0x04..=0x45       // A-Z (0x04..=0x1D), 1-0 (0x1E..=0x27), Enter/Esc/Bksp/Tab/Space/Punct (0x28..=0x38), CapsLock (0x39), F1-F12 (0x3A..=0x45)
        | 0x46..=0x52     // PrintScreen, ScrollLock, Pause, Insert, Home, PageUp, Delete, End, PageDown, Right, Left, Down, Up
        | 0x53..=0x65     // Keypad NumLock through Keypad Menu, plus NUBS (0x64)
        | 0x67            // Keypad =
        | 0xE0..=0xE7     // Modifiers (LCtrl, LShift, LAlt, LGUI, RCtrl, RShift, RAlt, RGUI)
    )
}

/// Returns `true` if a USB HID Keyboard Page (0x07) usage ID is an extended function key (F13-F24).
pub fn is_extended_fn_usage(usage_id: u16) -> bool {
    matches!(usage_id, 0x68..=0x73)
}

/// Returns `true` if a USB HID Keyboard Page (0x07) usage ID is an international or language switching key
/// (e.g. Int1-Int9, Lang1-Lang9).
pub fn is_international_usage(usage_id: u16) -> bool {
    matches!(usage_id, 0x87..=0x98)
}

/// Attaches search aliases for international and language switching keys.
pub fn attach_friendly_international_aliases(cand: Candidate, id: u16) -> Candidate {
    let aliases: &[&str] = match id {
        0x87 => &["int1", "ro", "backslash ro"],
        0x88 => &["int2", "katakanahiragana", "kana"],
        0x89 => &["int3", "yen"],
        0x8A => &["int4", "henkan"],
        0x8B => &["int5", "muhenkan"],
        0x8C => &["int6", "numpadcomma"],
        0x8D => &["int7"],
        0x8E => &["int8"],
        0x8F => &["int9"],
        0x90 => &["lang1", "hangul", "hanja"],
        0x91 => &["lang2", "hanja", "eisu"],
        0x92 => &["lang3", "katakana"],
        0x93 => &["lang4", "hiragana"],
        0x94 => &["lang5", "zenkakuhankaku"],
        0x95 => &["lang6"],
        0x96 => &["lang7"],
        0x97 => &["lang8"],
        0x98 => &["lang9"],
        _ => &[],
    };
    cand.with_search_tokens(aliases.iter().copied())
}

/// Builds candidates for the "Basic" keyboard group (USB HID Page 0x07), strictly limited to standard keys.
/// Includes Transparent and None at the beginning of the Basic key group.
pub fn build_keyboard_candidates(
    presenter: &dyn KeyPresenter,
    usages: impl IntoIterator<Item = u16>,
    token_fn: impl Fn(u16) -> Vec<String>,
    supported_fn: impl Fn(u16) -> bool,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();

    // Prepend Transparent and None to Basic
    candidates.push(action_candidate(
        KeySpec::Transparent,
        &["transparent", "trans", "pass"],
        presenter,
    ));
    candidates.push(action_candidate(
        KeySpec::None,
        &["none", "noop", "unbound"],
        presenter,
    ));

    candidates.extend(
        usages
            .into_iter()
            .filter(|&id| is_basic_keyboard_usage(id) && supported_fn(id))
            .map(|id| {
                let action = KeySpec::KeyPress {
                    key: HidKey::keyboard(id),
                    modifiers: Modifiers::default(),
                };
                let mut cand =
                    Candidate::from_action(action, presenter, &[]).with_search_tokens(token_fn(id));
                if cand.key.symbol.is_none() && cand.key.tap.is_empty() {
                    cand.key.symbol = Some(format!("0x{:02X}", id));
                }
                attach_friendly_keyboard_aliases(cand, id)
            }),
    );

    candidates
}

/// Builds candidates for the "International" language/switching group.
pub fn build_international_candidates(
    presenter: &dyn KeyPresenter,
    usages: impl IntoIterator<Item = u16>,
    token_fn: impl Fn(u16) -> Vec<String>,
    supported_fn: impl Fn(u16) -> bool,
) -> Vec<Candidate> {
    usages
        .into_iter()
        .filter(|&id| is_international_usage(id) && supported_fn(id))
        .map(|id| {
            let action = KeySpec::KeyPress {
                key: HidKey::keyboard(id),
                modifiers: Modifiers::default(),
            };
            let mut cand =
                Candidate::from_action(action, presenter, &[]).with_search_tokens(token_fn(id));
            if cand.key.symbol.is_none() && cand.key.tap.is_empty() {
                cand.key.symbol = Some(format!("0x{:02X}", id));
            }
            attach_friendly_international_aliases(cand, id)
        })
        .collect()
}

/// Returns `true` if a USB HID Consumer Page (0x0C) usage is an audio or video/playback control
/// (e.g. Mute, Volume Up/Down, Play/Pause, Next/Previous, Stop, Fast Forward, Rewind, Eject).
pub fn is_audio_video_consumer_usage(usage_id: u16) -> bool {
    matches!(
        usage_id,
        0xE2            // Mute
        | 0xE9          // Volume Up
        | 0xEA          // Volume Down
        | 0xCD          // Play / Pause
        | 0xB5          // Next Track
        | 0xB6          // Previous Track
        | 0xB7          // Stop
        | 0xB3          // Fast Forward
        | 0xB4          // Rewind
        | 0xB8 // Eject
    )
}

/// Returns `true` if a USB HID Consumer Page (0x0C) usage is an application launcher, browser control, or display brightness key.
pub fn is_app_and_web_consumer_usage(usage_id: u16) -> bool {
    !is_audio_video_consumer_usage(usage_id)
}

/// Builds candidates for consumer/media usages (USB HID Page 0x0C), filtered to audio & video/playback controls.
pub fn build_media_candidates(
    presenter: &dyn KeyPresenter,
    usages: impl IntoIterator<Item = u16>,
    token_fn: impl Fn(u16) -> Vec<String>,
    supported_fn: impl Fn(u16) -> bool,
) -> Vec<Candidate> {
    usages
        .into_iter()
        .filter(|&id| is_audio_video_consumer_usage(id) && supported_fn(id))
        .map(|id| {
            let action = KeySpec::KeyPress {
                key: HidKey::consumer(id),
                modifiers: Modifiers::default(),
            };
            let mut cand =
                Candidate::from_action(action, presenter, &[]).with_search_tokens(token_fn(id));
            if cand.key.symbol.is_none() && cand.key.tap.is_empty() {
                cand.key.symbol = Some(format!("0x{:04X}", id));
            }
            attach_friendly_media_aliases(cand, id)
        })
        .collect()
}

/// Builds candidates for application launchers, browser controls, and display controls.
pub fn build_app_and_web_candidates(
    presenter: &dyn KeyPresenter,
    usages: impl IntoIterator<Item = u16>,
    token_fn: impl Fn(u16) -> Vec<String>,
    supported_fn: impl Fn(u16) -> bool,
) -> Vec<Candidate> {
    usages
        .into_iter()
        .filter(|&id| is_app_and_web_consumer_usage(id) && supported_fn(id))
        .map(|id| {
            let action = KeySpec::KeyPress {
                key: HidKey::consumer(id),
                modifiers: Modifiers::default(),
            };
            let mut cand =
                Candidate::from_action(action, presenter, &[]).with_search_tokens(token_fn(id));
            if cand.key.symbol.is_none() && cand.key.tap.is_empty() {
                cand.key.symbol = Some(format!("0x{:04X}", id));
            }
            attach_friendly_media_aliases(cand, id)
        })
        .collect()
}

/// Builds the Bluetooth candidate group with optional extra firmware-specific tokens.
pub fn build_bluetooth_group(
    presenter: &dyn KeyPresenter,
    extra_tokens_fn: impl Fn(BluetoothAction) -> &'static [&'static str],
) -> CandidateGroup {
    let base_actions: &[(BluetoothAction, &[&str])] = &[
        (
            BluetoothAction::Clear,
            &["bt clear", "bt clr", "disconnect"],
        ),
        (BluetoothAction::Next, &["bt next", "bt nxt"]),
        (BluetoothAction::Prev, &["bt prev", "bt prv"]),
        (BluetoothAction::ClearAll, &["bt clear all", "bt clr all"]),
    ];

    let mut candidates: Vec<Candidate> = base_actions
        .iter()
        .map(|(act, names)| {
            action_candidate(KeySpec::Bluetooth(*act), names, presenter)
                .with_search_tokens(extra_tokens_fn(*act).iter().copied())
        })
        .collect();

    for i in 0..=9 {
        let action = BluetoothAction::Select(i);
        candidates.push(
            action_candidate(KeySpec::Bluetooth(action), &["bt sel"], presenter)
                .with_parameter(ParameterSpec::BluetoothProfile { max: 9 })
                .with_search_tokens([format!("bt {i}"), format!("bt sel {i}")])
                .with_search_tokens(extra_tokens_fn(action).iter().copied()),
        );
    }
    for i in 0..=9 {
        let action = BluetoothAction::Disconnect(i);
        candidates.push(
            action_candidate(KeySpec::Bluetooth(action), &["bt disc"], presenter)
                .with_parameter(ParameterSpec::BluetoothProfile { max: 9 })
                .with_search_token(format!("bt disc {i}"))
                .with_search_tokens(extra_tokens_fn(action).iter().copied()),
        );
    }

    CandidateGroup {
        name: "Bluetooth",
        candidates,
    }
}

/// Builds the Output candidate group with optional extra firmware-specific tokens.
pub fn build_output_group(
    presenter: &dyn KeyPresenter,
    extra_tokens_fn: impl Fn(OutputTarget) -> &'static [&'static str],
) -> CandidateGroup {
    let targets = [
        (OutputTarget::Toggle, &["output toggle", "out tog"][..]),
        (OutputTarget::Usb, &["output usb", "out usb"][..]),
        (OutputTarget::Ble, &["output ble", "out ble"][..]),
        (OutputTarget::None, &["output none", "out none"][..]),
    ];

    let candidates = targets
        .into_iter()
        .map(|(target, names)| {
            action_candidate(KeySpec::Output(target), names, presenter)
                .with_search_tokens(extra_tokens_fn(target).iter().copied())
        })
        .collect();

    CandidateGroup {
        name: "Output",
        candidates,
    }
}

/// Builds the System candidate group with optional extra firmware-specific tokens.
pub fn build_system_group(
    presenter: &dyn KeyPresenter,
    extra_tokens_fn: impl Fn(u16) -> &'static [&'static str],
) -> CandidateGroup {
    let sys_keys = [
        (
            HidKey::system(0x81),
            &["system power", "sys power", "power down"][..],
        ),
        (
            HidKey::system(0x82),
            &["system sleep", "sys sleep", "sleep"][..],
        ),
        (
            HidKey::system(0x83),
            &["system wake", "sys wake", "wake"][..],
        ),
    ];
    let candidates = sys_keys
        .into_iter()
        .map(|(key, names)| {
            action_candidate(
                KeySpec::KeyPress {
                    key,
                    modifiers: Modifiers::default(),
                },
                names,
                presenter,
            )
            .with_search_tokens(extra_tokens_fn(key.id).iter().copied())
        })
        .collect();

    CandidateGroup {
        name: "System",
        candidates,
    }
}

/// Builds the consolidated System & Power candidate group with firmware-specific tokens.
pub fn build_system_and_power_group(
    presenter: &dyn KeyPresenter,
    boot_power_actions: &[(PowerAction, &[&str])],
    system_tokens_fn: impl Fn(u16) -> &'static [&'static str],
    power_tokens_fn: impl Fn(PowerAction) -> &'static [&'static str],
) -> CandidateGroup {
    let mut candidates = build_system_group(presenter, system_tokens_fn).candidates;
    candidates
        .extend(build_boot_power_group(presenter, boot_power_actions, power_tokens_fn).candidates);
    CandidateGroup {
        name: "System & Power",
        candidates,
    }
}

/// Builds the Boot & Power candidate group with optional extra firmware-specific tokens.
pub fn build_boot_power_group(
    presenter: &dyn KeyPresenter,
    actions: &[(PowerAction, &[&str])],
    extra_tokens_fn: impl Fn(PowerAction) -> &'static [&'static str],
) -> CandidateGroup {
    let candidates = actions
        .iter()
        .map(|(action, names)| {
            action_candidate(KeySpec::Power(*action), names, presenter)
                .with_search_tokens(extra_tokens_fn(*action).iter().copied())
        })
        .collect();

    CandidateGroup {
        name: "Boot & Power",
        candidates,
    }
}

/// Builds the Backlight candidate group with optional extra firmware-specific tokens.
pub fn build_backlight_group(
    presenter: &dyn KeyPresenter,
    actions: &[BacklightAction],
    extra_tokens_fn: impl Fn(BacklightAction) -> &'static [&'static str],
) -> CandidateGroup {
    let items: &[(BacklightAction, &[&str], Option<ParameterSpec>)] = &[
        (BacklightAction::Toggle, &["bl toggle", "backlight"], None),
        (BacklightAction::On, &["bl on", "backlight on"], None),
        (BacklightAction::Off, &["bl off", "backlight off"], None),
        (BacklightAction::Inc, &["bl inc", "backlight up"], None),
        (BacklightAction::Dec, &["bl dec", "backlight down"], None),
        (
            BacklightAction::Cycle,
            &["bl step", "backlight cycle"],
            None,
        ),
        (
            BacklightAction::BreathingToggle,
            &["bl breath", "bl breathing"],
            None,
        ),
        (
            BacklightAction::Set(0),
            &["bl set", "backlight level", "brightness"],
            Some(ParameterSpec::Brightness { min: 0, max: 255 }),
        ),
    ];

    let candidates = items
        .iter()
        .filter(|(action, _, _)| actions.contains(action))
        .map(|(action, names, param)| {
            let mut cand = action_candidate(
                KeySpec::Lighting(LightingAction::Backlight(*action)),
                names,
                presenter,
            )
            .with_search_tokens(extra_tokens_fn(*action).iter().copied());
            if let Some(param) = *param {
                cand = cand.with_parameter(param);
            }
            cand
        })
        .collect();

    CandidateGroup {
        name: "Backlight",
        candidates,
    }
}

/// Builds the RGB Underglow candidate group with optional extra firmware-specific tokens.
pub fn build_rgb_underglow_group(
    presenter: &dyn KeyPresenter,
    actions: &[RgbAction],
    extra_tokens_fn: impl Fn(RgbAction) -> &'static [&'static str],
) -> CandidateGroup {
    let items: &[(RgbAction, &[&str], Option<ParameterSpec>)] = &[
        (RgbAction::Toggle, &["rgb toggle", "rgb tog"], None),
        (RgbAction::On, &["rgb on", "underglow on"], None),
        (RgbAction::Off, &["rgb off", "underglow off"], None),
        (RgbAction::EffectInc, &["rgb mode", "rgb next"], None),
        (RgbAction::EffectDec, &["rgb prev"], None),
        (RgbAction::HueInc, &["rgb hue+", "hue up"], None),
        (RgbAction::HueDec, &["rgb hue-", "hue down"], None),
        (RgbAction::SatInc, &["rgb sat+", "sat up"], None),
        (RgbAction::SatDec, &["rgb sat-", "sat down"], None),
        (RgbAction::BrightInc, &["rgb val+", "bri up"], None),
        (RgbAction::BrightDec, &["rgb val-", "bri down"], None),
        (RgbAction::SpeedInc, &["rgb speed+", "spd up"], None),
        (RgbAction::SpeedDec, &["rgb speed-", "spd down"], None),
        (
            RgbAction::EffectSet,
            &["rgb effect set", "eff set"],
            Some(ParameterSpec::EffectIndex),
        ),
        (
            RgbAction::Color,
            &["rgb color", "color"],
            Some(ParameterSpec::HsbColor),
        ),
    ];

    let candidates = items
        .iter()
        .filter(|(action, _, _)| actions.contains(action))
        .map(|(action, names, param)| {
            let mut cand = action_candidate(
                KeySpec::Lighting(LightingAction::Rgb(*action)),
                names,
                presenter,
            )
            .with_search_tokens(extra_tokens_fn(*action).iter().copied());
            if let Some(param) = *param {
                cand = cand.with_parameter(param);
            }
            cand
        })
        .collect();

    CandidateGroup {
        name: "RGB Underglow",
        candidates,
    }
}

/// Builds the RGB Matrix candidate group with optional extra firmware-specific tokens.
///
/// Includes On and Off candidates (`QK_RGB_MATRIX_ON` / `QK_RGB_MATRIX_OFF`, aliases `RM_ON`
/// / `RM_OFF`) which are valid QMK keycodes.
pub fn build_rgb_matrix_group(
    presenter: &dyn KeyPresenter,
    extra_tokens_fn: impl Fn(RgbMatrixAction) -> &'static [&'static str],
) -> CandidateGroup {
    let actions: &[(RgbMatrixAction, &[&str])] = &[
        (RgbMatrixAction::On, &["rgb matrix on", "rm_on"]),
        (RgbMatrixAction::Off, &["rgb matrix off", "rm_off"]),
        (RgbMatrixAction::Toggle, &["rgb matrix toggle", "rm_togg"]),
        (RgbMatrixAction::ModeNext, &["rgb matrix next", "rm_next"]),
        (RgbMatrixAction::ModePrev, &["rgb matrix prev", "rm_prev"]),
        (RgbMatrixAction::HueInc, &["rgb matrix hue+", "rm_hueu"]),
        (RgbMatrixAction::HueDec, &["rgb matrix hue-", "rm_hued"]),
        (RgbMatrixAction::SatInc, &["rgb matrix sat+", "rm_satu"]),
        (RgbMatrixAction::SatDec, &["rgb matrix sat-", "rm_satd"]),
        (RgbMatrixAction::BrightInc, &["rgb matrix val+", "rm_valu"]),
        (RgbMatrixAction::BrightDec, &["rgb matrix val-", "rm_vald"]),
        (RgbMatrixAction::SpeedInc, &["rgb matrix speed+", "rm_spdu"]),
        (RgbMatrixAction::SpeedDec, &["rgb matrix speed-", "rm_spdd"]),
    ];
    let candidates = actions
        .iter()
        .map(|(act, names)| {
            action_candidate(
                KeySpec::Lighting(LightingAction::RgbMatrix(*act)),
                names,
                presenter,
            )
            .with_search_tokens(extra_tokens_fn(*act).iter().copied())
        })
        .collect();
    CandidateGroup {
        name: "RGB Matrix",
        candidates,
    }
}

/// Builds the Audio candidate group with optional extra firmware-specific tokens.
pub fn build_audio_group(
    presenter: &dyn KeyPresenter,
    extra_tokens_fn: impl Fn(AudioAction) -> &'static [&'static str],
) -> CandidateGroup {
    let actions: &[(AudioAction, &[&str])] = &[
        (AudioAction::On, &["audio on", "au_on"]),
        (AudioAction::Off, &["audio off", "au_off"]),
        (AudioAction::Toggle, &["audio toggle", "au_tog"]),
        (AudioAction::ClickyToggle, &["clicky toggle", "ck_tog"]),
        (AudioAction::ClickyOn, &["clicky on", "ck_on"]),
        (AudioAction::ClickyOff, &["clicky off", "ck_off"]),
        (AudioAction::ClickyUp, &["clicky up", "ck_up"]),
        (AudioAction::ClickyDown, &["clicky down", "ck_down"]),
        (AudioAction::ClickyReset, &["clicky reset", "ck_rst"]),
        (AudioAction::MusicOn, &["music on", "mu_on"]),
        (AudioAction::MusicOff, &["music off", "mu_off"]),
        (AudioAction::MusicToggle, &["music toggle", "mu_tog"]),
        (AudioAction::MusicModeNext, &["music mode", "mu_mod"]),
        (AudioAction::VoiceNext, &["audio voice+", "voice next"]),
        (AudioAction::VoicePrev, &["audio voice-", "voice prev"]),
    ];
    let candidates = actions
        .iter()
        .map(|(act, names)| {
            action_candidate(KeySpec::Audio(*act), names, presenter)
                .with_search_tokens(extra_tokens_fn(*act).iter().copied())
        })
        .collect();
    CandidateGroup {
        name: "Audio",
        candidates,
    }
}

/// Builds the Mouse candidate groups (Buttons, Movement, Scroll, and optionally Acceleration).
pub fn build_mouse_groups(
    presenter: &dyn KeyPresenter,
    include_acceleration: bool,
    extra_tokens_fn: impl Fn(&KeySpec) -> &'static [&'static str],
) -> Vec<CandidateGroup> {
    let buttons = [
        (MouseButton::Left, &["mouse left", "btn1", "left click"][..]),
        (
            MouseButton::Right,
            &["mouse right", "btn2", "right click"][..],
        ),
        (
            MouseButton::Middle,
            &["mouse middle", "btn3", "middle click"][..],
        ),
        (MouseButton::Button4, &["mouse btn4"][..]),
        (MouseButton::Button5, &["mouse btn5"][..]),
        (MouseButton::Other(6), &["mouse btn6"][..]),
        (MouseButton::Other(7), &["mouse btn7"][..]),
        (MouseButton::Other(8), &["mouse btn8"][..]),
    ];

    let button_candidates = buttons
        .into_iter()
        .map(|(btn, names)| {
            let spec = KeySpec::Mouse(MouseAction::Press(btn));
            let extra = extra_tokens_fn(&spec);
            action_candidate(spec, names, presenter).with_search_tokens(extra.iter().copied())
        })
        .collect();

    let move_directions = [
        (
            MouseAction::Move { x: 0, y: -1 },
            &["mouse up", "move up", "ms up"][..],
        ),
        (
            MouseAction::Move { x: 0, y: 1 },
            &["mouse down", "move down", "ms down"][..],
        ),
        (
            MouseAction::Move { x: -1, y: 0 },
            &["mouse left", "move left", "ms left"][..],
        ),
        (
            MouseAction::Move { x: 1, y: 0 },
            &["mouse right", "move right", "ms right"][..],
        ),
    ];

    let move_candidates = move_directions
        .into_iter()
        .map(|(action, names)| {
            let spec = KeySpec::Mouse(action);
            let extra = extra_tokens_fn(&spec);
            action_candidate(spec, names, presenter).with_search_tokens(extra.iter().copied())
        })
        .collect();

    let scroll_directions = [
        (
            MouseAction::Scroll { x: 0, y: 1 },
            &["scroll up", "wh up"][..],
        ),
        (
            MouseAction::Scroll { x: 0, y: -1 },
            &["scroll down", "wh down"][..],
        ),
        (
            MouseAction::Scroll { x: -1, y: 0 },
            &["scroll left", "wh left"][..],
        ),
        (
            MouseAction::Scroll { x: 1, y: 0 },
            &["scroll right", "wh right"][..],
        ),
    ];

    let scroll_candidates = scroll_directions
        .into_iter()
        .map(|(action, names)| {
            let spec = KeySpec::Mouse(action);
            let extra = extra_tokens_fn(&spec);
            action_candidate(spec, names, presenter).with_search_tokens(extra.iter().copied())
        })
        .collect();

    let mut groups = vec![
        CandidateGroup {
            name: "Mouse Buttons",
            candidates: button_candidates,
        },
        CandidateGroup {
            name: "Mouse Movement",
            candidates: move_candidates,
        },
        CandidateGroup {
            name: "Mouse Scroll",
            candidates: scroll_candidates,
        },
    ];

    if include_acceleration {
        let accels = [
            (0u8, &["mouse accel 0", "accel 0"][..]),
            (1u8, &["mouse accel 1", "accel 1"][..]),
            (2u8, &["mouse accel 2", "accel 2"][..]),
        ];

        let accel_candidates = accels
            .into_iter()
            .map(|(level, names)| {
                let spec = KeySpec::Mouse(MouseAction::Acceleration(level));
                let extra = extra_tokens_fn(&spec);
                action_candidate(spec, names, presenter).with_search_tokens(extra.iter().copied())
            })
            .collect();

        groups.push(CandidateGroup {
            name: "Mouse Acceleration",
            candidates: accel_candidates,
        });
    }

    groups
}

/// Builds the Special candidate group with optional extra firmware-specific tokens.
pub fn build_special_group(
    presenter: &dyn KeyPresenter,
    extra_tokens_fn: impl Fn(&KeySpec) -> &'static [&'static str],
) -> CandidateGroup {
    let mut actions: Vec<(KeySpec, &[&str])> = vec![
        (KeySpec::CapsWord, &["caps word", "caps_word", "cw"]),
        (KeySpec::KeyRepeat, &["key repeat", "key_repeat", "repeat"]),
        (
            KeySpec::AltKeyRepeat,
            &["alt repeat", "alt_repeat", "arep", "repeat opposite"],
        ),
        (KeySpec::KeyLock, &["key lock", "lock key", "lock"]),
        (KeySpec::GraveEscape, &["grave escape", "grave_esc"]),
        (
            KeySpec::KeyPress {
                key: HidKey::keyboard(0x82),
                modifiers: Modifiers::default(),
            },
            &["locking caps lock", "locking caps", "lcap"],
        ),
        (
            KeySpec::KeyPress {
                key: HidKey::keyboard(0x83),
                modifiers: Modifiers::default(),
            },
            &["locking num lock", "locking num", "lnum"],
        ),
        (
            KeySpec::KeyPress {
                key: HidKey::keyboard(0x84),
                modifiers: Modifiers::default(),
            },
            &["locking scroll lock", "locking scroll", "lscrl"],
        ),
        (
            KeySpec::KeyPress {
                key: HidKey::keyboard(0x7A),
                modifiers: Modifiers::default(),
            },
            &["undo"],
        ),
        (
            KeySpec::KeyPress {
                key: HidKey::keyboard(0x7B),
                modifiers: Modifiers::default(),
            },
            &["cut"],
        ),
        (
            KeySpec::KeyPress {
                key: HidKey::keyboard(0x7C),
                modifiers: Modifiers::default(),
            },
            &["copy"],
        ),
        (
            KeySpec::KeyPress {
                key: HidKey::keyboard(0x7D),
                modifiers: Modifiers::default(),
            },
            &["paste"],
        ),
        (
            KeySpec::KeyPress {
                key: HidKey::keyboard(0x7E),
                modifiers: Modifiers::default(),
            },
            &["find"],
        ),
        (
            KeySpec::KeyPress {
                key: HidKey::keyboard(0x79),
                modifiers: Modifiers::default(),
            },
            &["again", "redo"],
        ),
        (
            KeySpec::KeyPress {
                key: HidKey::keyboard(0x78),
                modifiers: Modifiers::default(),
            },
            &["stop"],
        ),
        (
            KeySpec::KeyPress {
                key: HidKey::keyboard(0x66),
                modifiers: Modifiers::default(),
            },
            &["power", "mac power", "keyboard power"],
        ),
    ];

    // Extended Function Keys F13..=F24
    let f_aliases: [&[&str]; 12] = [
        &["f13"],
        &["f14"],
        &["f15"],
        &["f16"],
        &["f17"],
        &["f18"],
        &["f19"],
        &["f20"],
        &["f21"],
        &["f22"],
        &["f23"],
        &["f24"],
    ];
    for (i, usage) in (0x68..=0x73u16).enumerate() {
        actions.push((
            KeySpec::KeyPress {
                key: HidKey::keyboard(usage),
                modifiers: Modifiers::default(),
            },
            f_aliases[i],
        ));
    }

    // Specialized / Legacy Terminal Controls (0x99..=0xA4)
    let legacy_specials: &[(u16, &[&str])] = &[
        (0x99, &["alt erase"]),
        (0x9A, &["sysreq", "attention"]),
        (0x9B, &["cancel"]),
        (0x9C, &["clear"]),
        (0x9D, &["prior"]),
        (0x9E, &["return"]),
        (0x9F, &["separator"]),
        (0xA0, &["out"]),
        (0xA1, &["oper"]),
        (0xA2, &["clear again"]),
        (0xA3, &["crsel", "props"]),
        (0xA4, &["exsel"]),
    ];
    for &(usage, aliases) in legacy_specials {
        actions.push((
            KeySpec::KeyPress {
                key: HidKey::keyboard(usage),
                modifiers: Modifiers::default(),
            },
            aliases,
        ));
    }

    let candidates = actions
        .iter()
        .map(|(spec, names)| {
            let extra = extra_tokens_fn(spec);
            action_candidate(spec.clone(), names, presenter)
                .with_search_tokens(extra.iter().copied())
        })
        .collect();

    CandidateGroup {
        name: "Special",
        candidates,
    }
}

/// Builds candidate groups for user macros and firmware extensions.
pub fn build_custom_groups(presenter: &dyn KeyPresenter) -> Vec<CandidateGroup> {
    let kinds = [
        ("Macro", CustomKind::Macro),
        ("Tap Dance", CustomKind::TapDance),
        ("User", CustomKind::User),
        ("Keyboard", CustomKind::Keyboard),
    ];

    kinds
        .into_iter()
        .map(|(title, kind)| {
            let candidates = (0..32)
                .map(|i| {
                    let spec = KeySpec::Custom(CustomBinding {
                        kind,
                        id: i,
                        name: None,
                        param1: None,
                        param2: None,
                    });
                    Candidate::from_action(spec, presenter, &[])
                        .with_search_tokens([title, &format!("{title} {i}")])
                })
                .collect();

            CandidateGroup {
                name: title,
                candidates,
            }
        })
        .collect()
}

/// Candidate groups for supported layer operation types across real layers.
pub fn build_layer_groups(
    presenter: &dyn KeyPresenter,
    layer_count: usize,
    layer_names: &[String],
    tap: crate::keymap_editor::LayerTapTarget,
    ops: &[(&'static str, LayerActivation, &'static [&'static str])],
    extra_tokens_fn: impl Fn(LayerActivation) -> &'static [&'static str],
) -> Vec<CandidateGroup> {
    let count = layer_count.min(32);

    let mut groups: Vec<CandidateGroup> = ops
        .iter()
        .map(|(name, activation, search_tokens)| {
            let candidates = (0..count)
                .map(|layer| {
                    Candidate::from_action(
                        KeySpec::Layer {
                            layer: layer as u8,
                            activation: *activation,
                        },
                        presenter,
                        layer_names,
                    )
                    .with_parameter(ParameterSpec::Layer {
                        max: count.saturating_sub(1) as u8,
                    })
                    .with_search_tokens(search_tokens.iter().copied())
                    .with_search_tokens(extra_tokens_fn(*activation).iter().copied())
                    .with_search_token(format!("l{layer}"))
                })
                .collect();

            CandidateGroup { name, candidates }
        })
        .collect();

    let tap_key = tap.key.unwrap_or_else(|| HidKey::keyboard(0x2C));
    let lt_candidates = (0..count)
        .map(|layer| {
            Candidate::from_action(
                KeySpec::LayerTap {
                    layer: layer as u8,
                    tap: tap_key,
                    tap_modifiers: tap.modifiers,
                },
                presenter,
                layer_names,
            )
            .with_parameter(ParameterSpec::Layer {
                max: count.saturating_sub(1) as u8,
            })
            .with_search_tokens(["lt", "layer tap"])
            .with_search_token(format!("lt{layer}"))
            .with_search_token(format!("l{layer}"))
        })
        .collect();

    groups.push(CandidateGroup {
        name: "Layer Tap",
        candidates: lt_candidates,
    });

    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap_editor::LayerTapTarget;

    struct TestPresenter;
    impl KeyPresenter for TestPresenter {
        fn present_key(
            &self,
            _spec: &KeySpec,
            _layer_names: &[String],
        ) -> Option<crate::layout_key::LayoutKey> {
            Some(crate::layout_key::LayoutKey::default())
        }
    }

    #[test]
    fn common_keyboard_candidates_produce_expected_specs() {
        let cands = build_keyboard_candidates(
            &TestPresenter,
            vec![0x04, 0x28],
            |_| vec!["token".to_string()],
            |_| true,
        );
        assert_eq!(cands.len(), 4);
        assert_eq!(cands[0].binding, KeySpec::Transparent);
        assert_eq!(cands[1].binding, KeySpec::None);
        assert_eq!(
            cands[2].binding,
            KeySpec::KeyPress {
                key: HidKey::keyboard(0x04),
                modifiers: Modifiers::default(),
            }
        );
        assert!(cands[2].matches_query("token"));
        // Enter should get friendly alias
        assert!(cands[3].matches_query("enter"));
    }

    #[test]
    fn hid_candidates_honor_profile_support_predicate() {
        let cands = build_keyboard_candidates(
            &TestPresenter,
            vec![0x04, 0x28],
            |_| vec![],
            |id| id == 0x28,
        );

        assert_eq!(cands.len(), 3);
        assert!(matches!(
            cands[2].binding,
            KeySpec::KeyPress { key, .. } if key.id == 0x28
        ));
    }

    #[test]
    fn common_bluetooth_candidates_produce_bluetooth_specs() {
        let group = build_bluetooth_group(&TestPresenter, |_| &["test_extra"]);
        assert_eq!(group.name, "Bluetooth");
        assert!(group
            .candidates
            .iter()
            .any(|c| matches!(c.binding, KeySpec::Bluetooth(BluetoothAction::Clear))));
        assert!(group.candidates[0].matches_query("test_extra"));
        assert!(group
            .candidates
            .iter()
            .filter(|candidate| matches!(
                candidate.binding,
                KeySpec::Bluetooth(BluetoothAction::Select(_) | BluetoothAction::Disconnect(_))
            ))
            .all(
                |candidate| candidate.parameter == Some(ParameterSpec::BluetoothProfile { max: 9 })
            ));
    }

    #[test]
    fn layer_candidates_declare_available_layer_range() {
        let groups = build_layer_groups(
            &TestPresenter,
            3,
            &[],
            LayerTapTarget::default(),
            &[("Momentary", LayerActivation::Momentary, &[])],
            |_| &[],
        );

        assert!(groups
            .iter()
            .flat_map(|group| &group.candidates)
            .all(|candidate| { candidate.parameter == Some(ParameterSpec::Layer { max: 2 }) }));
    }

    #[test]
    fn common_international_candidates_produce_expected_specs() {
        let cands =
            build_international_candidates(&TestPresenter, vec![0x90, 0x88], |_| vec![], |_| true);
        assert_eq!(cands.len(), 2);
        assert_eq!(
            cands[0].binding,
            KeySpec::KeyPress {
                key: HidKey::keyboard(0x90),
                modifiers: Modifiers::default(),
            }
        );
        assert!(cands[0].matches_query("hangul"));
        assert!(cands[1].matches_query("kana"));
    }

    #[test]
    fn migrated_lighting_builders_preserve_filtering_and_tokens() {
        let backlight = build_backlight_group(
            &TestPresenter,
            &[BacklightAction::Toggle, BacklightAction::Cycle],
            |_| &["firmware backlight"],
        );
        assert_eq!(backlight.candidates.len(), 2);
        assert!(backlight.candidates[0].matches_query("bl toggle"));
        assert!(backlight.candidates[1].matches_query("firmware backlight"));
        assert!(backlight.candidates.iter().all(|candidate| matches!(
            candidate.binding,
            KeySpec::Lighting(LightingAction::Backlight(_))
        )));

        let zmk_backlight = build_backlight_group(
            &TestPresenter,
            &[
                BacklightAction::Toggle,
                BacklightAction::On,
                BacklightAction::Off,
                BacklightAction::Inc,
                BacklightAction::Dec,
                BacklightAction::Cycle,
                BacklightAction::Set(0),
            ],
            |_| &[],
        );
        let set_candidate = zmk_backlight
            .candidates
            .iter()
            .find(|candidate| {
                matches!(
                    candidate.binding,
                    KeySpec::Lighting(LightingAction::Backlight(BacklightAction::Set(0)))
                )
            })
            .expect("ZMK backlight should expose BL_SET");
        assert_eq!(
            set_candidate.parameter,
            Some(ParameterSpec::Brightness { min: 0, max: 255 })
        );

        let rgb = build_rgb_underglow_group(
            &TestPresenter,
            &[RgbAction::Toggle, RgbAction::Color],
            |_| &["firmware rgb"],
        );
        assert_eq!(rgb.candidates.len(), 2);
        assert!(rgb
            .candidates
            .iter()
            .all(|candidate| candidate.matches_query("firmware rgb")));

        let rgb_matrix = build_rgb_matrix_group(&TestPresenter, |_| &["firmware matrix"]);
        assert_eq!(rgb_matrix.candidates.len(), 13);
        assert!(rgb_matrix.candidates[0].matches_query("rm_on"));
        assert!(rgb_matrix.candidates[0].matches_query("firmware matrix"));

        let audio = build_audio_group(&TestPresenter, |_| &["firmware audio"]);
        assert_eq!(audio.candidates.len(), 15);
        assert!(audio.candidates[0].matches_query("au_on"));
        assert!(audio.candidates[0].matches_query("firmware audio"));
    }
}
