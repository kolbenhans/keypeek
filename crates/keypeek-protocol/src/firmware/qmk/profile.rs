use std::sync::OnceLock;

use crate::firmware::qmk::QmkKeyPresenter;
use keypeek_core::keymap_editor::catalog::common;
use keypeek_core::keymap_editor::draft::EditorSection;
use keypeek_core::keymap_editor::profile::{EditorProfile, SidebarSection};
use keypeek_core::keymap_editor::{Candidate, CandidateGroup, FeatureManifest, LayerTapTarget};
use keypeek_core::{
    AudioAction, AutoShiftAction, BacklightAction, ComboAction, CustomBinding, CustomKind, HidKey,
    KeyPresenter, KeySpec, LayerActivation, LayoutKey, Modifiers, MouseAction, MouseButton,
    OneShotAction, PowerAction, RgbAction, RgbMatrixAction, SpaceCadet,
};
use qmk_via_api::keycodes::Keycode;

const QMK_SIDEBAR_SECTIONS: [SidebarSection<FeatureManifest>; 4] = [
    SidebarSection {
        title: "Keys",
        items: &[
            FeatureManifest::supported(EditorSection::Keyboard),
            FeatureManifest::supported(EditorSection::Media),
            FeatureManifest::supported(EditorSection::Special),
        ],
    },
    SidebarSection {
        title: "Layers & Mods",
        items: &[
            FeatureManifest::supported(EditorSection::Layers),
            FeatureManifest::supported(EditorSection::Combo),
            FeatureManifest::supported(EditorSection::OneShot),
            FeatureManifest::supported(EditorSection::ModTap),
            FeatureManifest::supported(EditorSection::LayerMod),
        ],
    },
    SidebarSection {
        title: "Lighting & Audio",
        items: &[
            FeatureManifest::conditional(EditorSection::Backlight),
            FeatureManifest::conditional(EditorSection::Rgb),
            FeatureManifest::conditional(EditorSection::RgbMatrix),
            FeatureManifest::conditional(EditorSection::Audio),
        ],
    },
    SidebarSection {
        title: "Other",
        items: &[
            FeatureManifest::conditional(EditorSection::Mouse),
            FeatureManifest::supported(EditorSection::System),
            FeatureManifest::conditional(EditorSection::Custom),
            FeatureManifest::supported(EditorSection::RawHex),
        ],
    },
];

#[derive(Debug, Default, Clone, Copy)]
pub struct QmkEditorProfile;

const QMK_BACKLIGHT_ACTIONS: &[BacklightAction] = &[
    BacklightAction::Toggle,
    BacklightAction::On,
    BacklightAction::Off,
    BacklightAction::Inc,
    BacklightAction::Dec,
    BacklightAction::Cycle,
    BacklightAction::BreathingToggle,
];

const QMK_RGB_UNDERGLOW_ACTIONS: &[RgbAction] = &[
    RgbAction::Toggle,
    RgbAction::EffectInc,
    RgbAction::EffectDec,
    RgbAction::HueInc,
    RgbAction::HueDec,
    RgbAction::SatInc,
    RgbAction::SatDec,
    RgbAction::BrightInc,
    RgbAction::BrightDec,
    RgbAction::SpeedInc,
    RgbAction::SpeedDec,
];

fn qmk_supports_hid(page: u16, id: u16) -> bool {
    let key = match page {
        0x07 => HidKey::keyboard(id),
        0x0C => HidKey::consumer(id),
        _ => return false,
    };
    super::codec::keyspec_to_qmk(&KeySpec::KeyPress {
        key,
        modifiers: Modifiers::default(),
    })
    .is_ok()
}

fn qmk_keyboard_group() -> &'static CandidateGroup {
    static GROUP: OnceLock<CandidateGroup> = OnceLock::new();
    GROUP.get_or_init(|| {
        let usages = super::codec::qmk_all_basic_usages();
        let candidates = common::build_keyboard_candidates(
            &QmkKeyPresenter,
            usages,
            |id| super::codec::qmk_search_tokens_for_hid(0x07, id),
            |id| qmk_supports_hid(0x07, id),
        );
        CandidateGroup {
            name: "Basic",
            candidates,
        }
    })
}

fn qmk_media_group() -> &'static CandidateGroup {
    static GROUP: OnceLock<CandidateGroup> = OnceLock::new();
    GROUP.get_or_init(|| {
        let usages = super::codec::qmk_all_media_usages();
        let candidates = common::build_media_candidates(
            &QmkKeyPresenter,
            usages,
            |id| super::codec::qmk_search_tokens_for_hid(0x0C, id),
            |id| qmk_supports_hid(0x0C, id),
        );
        CandidateGroup {
            name: "Media",
            candidates,
        }
    })
}

fn qmk_apps_and_web_group() -> &'static CandidateGroup {
    static GROUP: OnceLock<CandidateGroup> = OnceLock::new();
    GROUP.get_or_init(|| {
        let usages = super::codec::qmk_all_media_usages();
        let candidates = common::build_app_and_web_candidates(
            &QmkKeyPresenter,
            usages,
            |id| super::codec::qmk_search_tokens_for_hid(0x0C, id),
            |id| qmk_supports_hid(0x0C, id),
        );
        CandidateGroup {
            name: "Apps & Web",
            candidates,
        }
    })
}

fn qmk_international_group() -> &'static CandidateGroup {
    static GROUP: OnceLock<CandidateGroup> = OnceLock::new();
    GROUP.get_or_init(|| {
        let usages = super::codec::qmk_all_basic_usages();
        let candidates = common::build_international_candidates(
            &QmkKeyPresenter,
            usages,
            |id| super::codec::qmk_search_tokens_for_hid(0x07, id),
            |id| qmk_supports_hid(0x07, id),
        );
        CandidateGroup {
            name: "International",
            candidates,
        }
    })
}

const QMK_BOOT_POWER_ACTIONS: [(PowerAction, &[&str]); 3] = [
    (PowerAction::Reset, &["reset", "reboot"]),
    (
        PowerAction::Bootloader,
        &["bootloader", "dfu", "flash", "boot"],
    ),
    (PowerAction::Other(0xEE), &["clear eeprom", "eeprom reset"]),
];

fn qmk_system_and_power_group() -> &'static CandidateGroup {
    static GROUP: OnceLock<CandidateGroup> = OnceLock::new();
    GROUP.get_or_init(|| {
        common::build_system_and_power_group(
            &QmkKeyPresenter,
            &QMK_BOOT_POWER_ACTIONS,
            |id| match id {
                0x81 => &["KC_SYSTEM_POWER"][..],
                0x82 => &["KC_SYSTEM_SLEEP"][..],
                0x83 => &["KC_SYSTEM_WAKE"][..],
                _ => &[][..],
            },
            |act| match act {
                PowerAction::Reset => &["QK_BOOT", "QK_REBOOT"],
                PowerAction::Bootloader => &["QK_BOOTLOADER"],
                PowerAction::Other(0xEE) => &["QK_CLEAR_EEPROM"],
                _ => &[],
            },
        )
    })
}

fn qmk_tap_categories() -> &'static [CandidateGroup] {
    static CATEGORIES: OnceLock<Vec<CandidateGroup>> = OnceLock::new();
    CATEGORIES.get_or_init(|| {
        vec![
            qmk_keyboard_group().clone(),
            qmk_special_groups()[0].clone(),
            qmk_space_cadet_group().clone(),
            qmk_auto_shift_group().clone(),
            qmk_combo_group().clone(),
            qmk_one_shot_control_group().clone(),
            qmk_oneshot_mod_group().clone(),
            qmk_international_group().clone(),
        ]
    })
}

fn qmk_media_groups() -> &'static [CandidateGroup] {
    static GROUPS: OnceLock<Vec<CandidateGroup>> = OnceLock::new();
    GROUPS.get_or_init(|| vec![qmk_media_group().clone(), qmk_apps_and_web_group().clone()])
}

fn qmk_raw_group(name: &'static str, items: &[(Keycode, &[&str])]) -> CandidateGroup {
    let candidates = items
        .iter()
        .map(|(keycode, tokens)| {
            let code = *keycode as u16;
            let binding = KeySpec::Custom(CustomBinding {
                kind: CustomKind::Raw,
                id: code as u32,
                name: super::keycode_labels::try_resolve_qmk_key(code).map(|key| key.tap.full),
                param1: None,
                param2: None,
            });
            Candidate::from_action(binding, &QmkKeyPresenter, &[])
                .with_search_tokens(tokens.iter().copied())
        })
        .collect();

    CandidateGroup { name, candidates }
}

fn qmk_extended_qmk_groups() -> &'static [CandidateGroup] {
    static GROUPS: OnceLock<Vec<CandidateGroup>> = OnceLock::new();
    GROUPS.get_or_init(|| {
        const UNICODE: &[(Keycode, &[&str])] = &[
            (
                Keycode::QK_UNICODE_MODE_NEXT,
                &["QK_UNICODE_MODE_NEXT", "unicode next"],
            ),
            (
                Keycode::QK_UNICODE_MODE_PREVIOUS,
                &["QK_UNICODE_MODE_PREVIOUS", "unicode previous"],
            ),
            (
                Keycode::QK_UNICODE_MODE_MACOS,
                &["QK_UNICODE_MODE_MACOS", "unicode macos"],
            ),
            (
                Keycode::QK_UNICODE_MODE_LINUX,
                &["QK_UNICODE_MODE_LINUX", "unicode linux"],
            ),
            (
                Keycode::QK_UNICODE_MODE_WINDOWS,
                &["QK_UNICODE_MODE_WINDOWS", "unicode windows"],
            ),
            (
                Keycode::QK_UNICODE_MODE_BSD,
                &["QK_UNICODE_MODE_BSD", "unicode bsd"],
            ),
            (
                Keycode::QK_UNICODE_MODE_WINCOMPOSE,
                &["QK_UNICODE_MODE_WINCOMPOSE", "unicode wincompose"],
            ),
            (
                Keycode::QK_UNICODE_MODE_EMACS,
                &["QK_UNICODE_MODE_EMACS", "unicode emacs"],
            ),
        ];
        const AUTOCORRECT: &[(Keycode, &[&str])] = &[
            (
                Keycode::QK_AUTOCORRECT_ON,
                &["QK_AUTOCORRECT_ON", "autocorrect on"],
            ),
            (
                Keycode::QK_AUTOCORRECT_OFF,
                &["QK_AUTOCORRECT_OFF", "autocorrect off"],
            ),
            (
                Keycode::QK_AUTOCORRECT_TOGGLE,
                &["QK_AUTOCORRECT_TOGGLE", "autocorrect toggle"],
            ),
        ];
        const LEADER: &[(Keycode, &[&str])] = &[(Keycode::QK_LEADER, &["QK_LEADER", "leader"])];
        const SWAP_HANDS: &[(Keycode, &[&str])] = &[
            (
                Keycode::QK_SWAP_HANDS_TOGGLE,
                &["QK_SWAP_HANDS_TOGGLE", "swap hands toggle"],
            ),
            (
                Keycode::QK_SWAP_HANDS_TAP_TOGGLE,
                &["QK_SWAP_HANDS_TAP_TOGGLE", "swap hands tap toggle"],
            ),
            (
                Keycode::QK_SWAP_HANDS_MOMENTARY_ON,
                &["QK_SWAP_HANDS_MOMENTARY_ON", "swap hands on"],
            ),
            (
                Keycode::QK_SWAP_HANDS_MOMENTARY_OFF,
                &["QK_SWAP_HANDS_MOMENTARY_OFF", "swap hands off"],
            ),
            (
                Keycode::QK_SWAP_HANDS_OFF,
                &["QK_SWAP_HANDS_OFF", "swap hands off"],
            ),
            (
                Keycode::QK_SWAP_HANDS_ON,
                &["QK_SWAP_HANDS_ON", "swap hands on"],
            ),
            (
                Keycode::QK_SWAP_HANDS_ONE_SHOT,
                &["QK_SWAP_HANDS_ONE_SHOT", "swap hands one shot"],
            ),
        ];
        const MAGIC: &[(Keycode, &[&str])] = &[
            (
                Keycode::QK_MAGIC_SWAP_CONTROL_CAPS_LOCK,
                &["QK_MAGIC_SWAP_CONTROL_CAPS_LOCK", "magic swap ctrl caps"],
            ),
            (
                Keycode::QK_MAGIC_UNSWAP_CONTROL_CAPS_LOCK,
                &[
                    "QK_MAGIC_UNSWAP_CONTROL_CAPS_LOCK",
                    "magic unswap ctrl caps",
                ],
            ),
            (
                Keycode::QK_MAGIC_TOGGLE_CONTROL_CAPS_LOCK,
                &[
                    "QK_MAGIC_TOGGLE_CONTROL_CAPS_LOCK",
                    "magic toggle ctrl caps",
                ],
            ),
            (
                Keycode::QK_MAGIC_CAPS_LOCK_AS_CONTROL_OFF,
                &[
                    "QK_MAGIC_CAPS_LOCK_AS_CONTROL_OFF",
                    "magic caps as ctrl off",
                ],
            ),
            (
                Keycode::QK_MAGIC_CAPS_LOCK_AS_CONTROL_ON,
                &["QK_MAGIC_CAPS_LOCK_AS_CONTROL_ON", "magic caps as ctrl on"],
            ),
            (
                Keycode::QK_MAGIC_SWAP_LALT_LGUI,
                &["QK_MAGIC_SWAP_LALT_LGUI", "magic swap lalt lgui"],
            ),
            (
                Keycode::QK_MAGIC_UNSWAP_LALT_LGUI,
                &["QK_MAGIC_UNSWAP_LALT_LGUI", "magic unswap lalt lgui"],
            ),
            (
                Keycode::QK_MAGIC_SWAP_RALT_RGUI,
                &["QK_MAGIC_SWAP_RALT_RGUI", "magic swap ralt rgui"],
            ),
            (
                Keycode::QK_MAGIC_UNSWAP_RALT_RGUI,
                &["QK_MAGIC_UNSWAP_RALT_RGUI", "magic unswap ralt rgui"],
            ),
            (
                Keycode::QK_MAGIC_GUI_ON,
                &["QK_MAGIC_GUI_ON", "magic gui on"],
            ),
            (
                Keycode::QK_MAGIC_GUI_OFF,
                &["QK_MAGIC_GUI_OFF", "magic gui off"],
            ),
            (
                Keycode::QK_MAGIC_TOGGLE_GUI,
                &["QK_MAGIC_TOGGLE_GUI", "magic gui toggle"],
            ),
            (
                Keycode::QK_MAGIC_SWAP_GRAVE_ESC,
                &["QK_MAGIC_SWAP_GRAVE_ESC", "magic swap grave escape"],
            ),
            (
                Keycode::QK_MAGIC_UNSWAP_GRAVE_ESC,
                &["QK_MAGIC_UNSWAP_GRAVE_ESC", "magic unswap grave escape"],
            ),
            (
                Keycode::QK_MAGIC_SWAP_BACKSLASH_BACKSPACE,
                &[
                    "QK_MAGIC_SWAP_BACKSLASH_BACKSPACE",
                    "magic swap backslash backspace",
                ],
            ),
            (
                Keycode::QK_MAGIC_UNSWAP_BACKSLASH_BACKSPACE,
                &[
                    "QK_MAGIC_UNSWAP_BACKSLASH_BACKSPACE",
                    "magic unswap backslash backspace",
                ],
            ),
            (
                Keycode::QK_MAGIC_TOGGLE_BACKSLASH_BACKSPACE,
                &[
                    "QK_MAGIC_TOGGLE_BACKSLASH_BACKSPACE",
                    "magic toggle backslash backspace",
                ],
            ),
            (
                Keycode::QK_MAGIC_NKRO_ON,
                &["QK_MAGIC_NKRO_ON", "magic nkro on"],
            ),
            (
                Keycode::QK_MAGIC_NKRO_OFF,
                &["QK_MAGIC_NKRO_OFF", "magic nkro off"],
            ),
            (
                Keycode::QK_MAGIC_TOGGLE_NKRO,
                &["QK_MAGIC_TOGGLE_NKRO", "magic nkro toggle"],
            ),
            (
                Keycode::QK_MAGIC_SWAP_ALT_GUI,
                &["QK_MAGIC_SWAP_ALT_GUI", "magic swap alt gui"],
            ),
            (
                Keycode::QK_MAGIC_UNSWAP_ALT_GUI,
                &["QK_MAGIC_UNSWAP_ALT_GUI", "magic unswap alt gui"],
            ),
            (
                Keycode::QK_MAGIC_TOGGLE_ALT_GUI,
                &["QK_MAGIC_TOGGLE_ALT_GUI", "magic toggle alt gui"],
            ),
            (
                Keycode::QK_MAGIC_SWAP_LCTL_LGUI,
                &["QK_MAGIC_SWAP_LCTL_LGUI", "magic swap lctl lgui"],
            ),
            (
                Keycode::QK_MAGIC_UNSWAP_LCTL_LGUI,
                &["QK_MAGIC_UNSWAP_LCTL_LGUI", "magic unswap lctl lgui"],
            ),
            (
                Keycode::QK_MAGIC_SWAP_RCTL_RGUI,
                &["QK_MAGIC_SWAP_RCTL_RGUI", "magic swap rctl rgui"],
            ),
            (
                Keycode::QK_MAGIC_UNSWAP_RCTL_RGUI,
                &["QK_MAGIC_UNSWAP_RCTL_RGUI", "magic unswap rctl rgui"],
            ),
            (
                Keycode::QK_MAGIC_SWAP_CTL_GUI,
                &["QK_MAGIC_SWAP_CTL_GUI", "magic swap ctrl gui"],
            ),
            (
                Keycode::QK_MAGIC_UNSWAP_CTL_GUI,
                &["QK_MAGIC_UNSWAP_CTL_GUI", "magic unswap ctrl gui"],
            ),
            (
                Keycode::QK_MAGIC_TOGGLE_CTL_GUI,
                &["QK_MAGIC_TOGGLE_CTL_GUI", "magic toggle ctrl gui"],
            ),
            (
                Keycode::QK_MAGIC_EE_HANDS_LEFT,
                &["QK_MAGIC_EE_HANDS_LEFT", "magic ee hands left"],
            ),
            (
                Keycode::QK_MAGIC_EE_HANDS_RIGHT,
                &["QK_MAGIC_EE_HANDS_RIGHT", "magic ee hands right"],
            ),
            (
                Keycode::QK_MAGIC_SWAP_ESCAPE_CAPS_LOCK,
                &["QK_MAGIC_SWAP_ESCAPE_CAPS_LOCK", "magic swap escape caps"],
            ),
            (
                Keycode::QK_MAGIC_UNSWAP_ESCAPE_CAPS_LOCK,
                &[
                    "QK_MAGIC_UNSWAP_ESCAPE_CAPS_LOCK",
                    "magic unswap escape caps",
                ],
            ),
            (
                Keycode::QK_MAGIC_TOGGLE_ESCAPE_CAPS_LOCK,
                &[
                    "QK_MAGIC_TOGGLE_ESCAPE_CAPS_LOCK",
                    "magic toggle escape caps",
                ],
            ),
        ];

        vec![
            qmk_raw_group("Unicode", UNICODE),
            qmk_raw_group("Autocorrect", AUTOCORRECT),
            qmk_raw_group("Leader", LEADER),
            qmk_raw_group("Swap Hands", SWAP_HANDS),
            qmk_raw_group("Magic", MAGIC),
        ]
    })
}

fn qmk_backlight_groups() -> &'static [CandidateGroup] {
    static GROUPS: OnceLock<Vec<CandidateGroup>> = OnceLock::new();
    GROUPS.get_or_init(|| {
        vec![common::build_backlight_group(
            &QmkKeyPresenter,
            QMK_BACKLIGHT_ACTIONS,
            |act| -> &'static [&'static str] {
                match act {
                    BacklightAction::Toggle => &["BL_TOGG", "QK_BACKLIGHT_TOGGLE"],
                    BacklightAction::On => &["BL_ON", "QK_BACKLIGHT_ON"],
                    BacklightAction::Off => &["BL_OFF", "QK_BACKLIGHT_OFF"],
                    BacklightAction::Inc => &["BL_UP", "QK_BACKLIGHT_UP"],
                    BacklightAction::Dec => &["BL_DOWN", "QK_BACKLIGHT_DOWN"],
                    BacklightAction::Cycle => &["BL_STEP", "QK_BACKLIGHT_STEP"],
                    BacklightAction::BreathingToggle => {
                        &["BL_BRTG", "QK_BACKLIGHT_TOGGLE_BREATHING"]
                    }
                    _ => &[],
                }
            },
        )]
    })
}

fn qmk_rgb_groups() -> &'static [CandidateGroup] {
    static GROUPS: OnceLock<Vec<CandidateGroup>> = OnceLock::new();
    GROUPS.get_or_init(|| {
        vec![common::build_rgb_underglow_group(
            &QmkKeyPresenter,
            QMK_RGB_UNDERGLOW_ACTIONS,
            |act| -> &'static [&'static str] {
                match act {
                    RgbAction::Toggle => &["RGB_TOG", "UG_TOGG", "QK_UNDERGLOW_TOGGLE"],
                    RgbAction::EffectInc => &["RGB_MOD", "UG_NEXT", "QK_UNDERGLOW_MODE_NEXT"],
                    RgbAction::EffectDec => &["RGB_RMOD", "UG_PREV", "QK_UNDERGLOW_MODE_PREVIOUS"],
                    RgbAction::HueInc => &["RGB_HUI", "QK_UNDERGLOW_HUE_UP"],
                    RgbAction::HueDec => &["RGB_HUD", "QK_UNDERGLOW_HUE_DOWN"],
                    RgbAction::SatInc => &["RGB_SAI", "QK_UNDERGLOW_SATURATION_UP"],
                    RgbAction::SatDec => &["RGB_SAD", "QK_UNDERGLOW_SATURATION_DOWN"],
                    RgbAction::BrightInc => &["RGB_VAI", "QK_UNDERGLOW_VALUE_UP"],
                    RgbAction::BrightDec => &["RGB_VAD", "QK_UNDERGLOW_VALUE_DOWN"],
                    RgbAction::SpeedInc => &["RGB_SPI", "QK_UNDERGLOW_SPEED_UP"],
                    RgbAction::SpeedDec => &["RGB_SPD", "QK_UNDERGLOW_SPEED_DOWN"],
                    _ => &[],
                }
            },
        )]
    })
}

fn qmk_rgb_matrix_groups() -> &'static [CandidateGroup] {
    static GROUPS: OnceLock<Vec<CandidateGroup>> = OnceLock::new();
    GROUPS.get_or_init(|| {
        vec![common::build_rgb_matrix_group(
            &QmkKeyPresenter,
            |act| -> &'static [&'static str] {
                match act {
                    RgbMatrixAction::On => &["RM_ON", "QK_RGB_MATRIX_ON"],
                    RgbMatrixAction::Off => &["RM_OFF", "QK_RGB_MATRIX_OFF"],
                    RgbMatrixAction::Toggle => &["RM_TOGG", "QK_RGB_MATRIX_TOGGLE"],
                    RgbMatrixAction::ModeNext => &["RM_NEXT", "QK_RGB_MATRIX_MODE_NEXT"],
                    RgbMatrixAction::ModePrev => &["RM_PREV", "QK_RGB_MATRIX_MODE_PREVIOUS"],
                    RgbMatrixAction::HueInc => &["RM_HUEU", "QK_RGB_MATRIX_HUE_UP"],
                    RgbMatrixAction::HueDec => &["RM_HUED", "QK_RGB_MATRIX_HUE_DOWN"],
                    RgbMatrixAction::SatInc => &["RM_SATU", "QK_RGB_MATRIX_SATURATION_UP"],
                    RgbMatrixAction::SatDec => &["RM_SATD", "QK_RGB_MATRIX_SATURATION_DOWN"],
                    RgbMatrixAction::BrightInc => &["RM_VALU", "QK_RGB_MATRIX_VALUE_UP"],
                    RgbMatrixAction::BrightDec => &["RM_VALD", "QK_RGB_MATRIX_VALUE_DOWN"],
                    RgbMatrixAction::SpeedInc => &["RM_SPDU", "QK_RGB_MATRIX_SPEED_UP"],
                    RgbMatrixAction::SpeedDec => &["RM_SPDD", "QK_RGB_MATRIX_SPEED_DOWN"],
                    _ => &[],
                }
            },
        )]
    })
}

fn qmk_audio_groups() -> &'static [CandidateGroup] {
    static GROUPS: OnceLock<Vec<CandidateGroup>> = OnceLock::new();
    GROUPS.get_or_init(|| {
        vec![common::build_audio_group(
            &QmkKeyPresenter,
            |act| -> &'static [&'static str] {
                match act {
                    AudioAction::On => &["QK_AUDIO_ON"],
                    AudioAction::Off => &["QK_AUDIO_OFF"],
                    AudioAction::Toggle => &["QK_AUDIO_TOGGLE"],
                    AudioAction::ClickyToggle => &["QK_AUDIO_CLICKY_TOGGLE"],
                    AudioAction::ClickyOn => &["QK_AUDIO_CLICKY_ON"],
                    AudioAction::ClickyOff => &["QK_AUDIO_CLICKY_OFF"],
                    AudioAction::ClickyUp => &["QK_AUDIO_CLICKY_UP"],
                    AudioAction::ClickyDown => &["QK_AUDIO_CLICKY_DOWN"],
                    AudioAction::ClickyReset => &["QK_AUDIO_CLICKY_RESET"],
                    AudioAction::MusicOn => &["QK_MUSIC_ON"],
                    AudioAction::MusicOff => &["QK_MUSIC_OFF"],
                    AudioAction::MusicToggle => &["QK_MUSIC_TOGGLE"],
                    AudioAction::MusicModeNext => &["QK_MUSIC_MODE_NEXT"],
                    AudioAction::VoiceNext => &["QK_AUDIO_VOICE_NEXT"],
                    AudioAction::VoicePrev => &["QK_AUDIO_VOICE_PREVIOUS"],
                    _ => &[],
                }
            },
        )]
    })
}

fn qmk_mouse_groups() -> &'static [CandidateGroup] {
    static GROUPS: OnceLock<Vec<CandidateGroup>> = OnceLock::new();
    GROUPS.get_or_init(|| {
        common::build_mouse_groups(&QmkKeyPresenter, true, |spec| -> &'static [&'static str] {
            match spec {
                KeySpec::Mouse(MouseAction::Press(MouseButton::Left)) => &["MS_BTN1"],
                KeySpec::Mouse(MouseAction::Press(MouseButton::Right)) => &["MS_BTN2"],
                KeySpec::Mouse(MouseAction::Press(MouseButton::Middle)) => &["MS_BTN3"],
                KeySpec::Mouse(MouseAction::Press(MouseButton::Button4)) => &["MS_BTN4"],
                KeySpec::Mouse(MouseAction::Press(MouseButton::Button5)) => &["MS_BTN5"],
                KeySpec::Mouse(MouseAction::Press(MouseButton::Other(6))) => &["MS_BTN6"],
                KeySpec::Mouse(MouseAction::Press(MouseButton::Other(7))) => &["MS_BTN7"],
                KeySpec::Mouse(MouseAction::Press(MouseButton::Other(8))) => &["MS_BTN8"],
                KeySpec::Mouse(MouseAction::Move { x: 0, y: -1 }) => &["MS_UP"],
                KeySpec::Mouse(MouseAction::Move { x: 0, y: 1 }) => &["MS_DOWN"],
                KeySpec::Mouse(MouseAction::Move { x: -1, y: 0 }) => &["MS_LEFT"],
                KeySpec::Mouse(MouseAction::Move { x: 1, y: 0 }) => &["MS_RIGHT"],
                KeySpec::Mouse(MouseAction::Scroll { x: 0, y: 1 }) => &["MS_WH_UP", "MS_WHLU"],
                KeySpec::Mouse(MouseAction::Scroll { x: 0, y: -1 }) => &["MS_WH_DOWN", "MS_WHLD"],
                KeySpec::Mouse(MouseAction::Scroll { x: -1, y: 0 }) => &["MS_WH_LEFT", "MS_WHLL"],
                KeySpec::Mouse(MouseAction::Scroll { x: 1, y: 0 }) => &["MS_WH_RIGHT", "MS_WHLR"],
                KeySpec::Mouse(MouseAction::Acceleration(0)) => &["MS_ACL0"],
                KeySpec::Mouse(MouseAction::Acceleration(1)) => &["MS_ACL1"],
                KeySpec::Mouse(MouseAction::Acceleration(2)) => &["MS_ACL2"],
                _ => &[],
            }
        })
    })
}

fn build_qmk_group(name: &'static str, items: &[(KeySpec, &[&'static str])]) -> CandidateGroup {
    let candidates = items
        .iter()
        .map(|(spec, tokens)| {
            Candidate::from_action(spec.clone(), &QmkKeyPresenter, &[])
                .with_search_tokens(tokens.iter().copied())
        })
        .collect();

    CandidateGroup { name, candidates }
}

fn qmk_space_cadet_group() -> &'static CandidateGroup {
    static GROUP: OnceLock<CandidateGroup> = OnceLock::new();
    GROUP.get_or_init(|| {
        build_qmk_group(
            "Space Cadet",
            &[
                (
                    KeySpec::SpaceCadet(SpaceCadet::LeftShiftOpenParen),
                    &[
                        "KC_LSPO",
                        "LSPO",
                        "space cadet left shift",
                        "shift (",
                        "shift paren",
                    ],
                ),
                (
                    KeySpec::SpaceCadet(SpaceCadet::RightShiftCloseParen),
                    &[
                        "KC_RSPC",
                        "RSPC",
                        "space cadet right shift",
                        "shift )",
                        "shift paren",
                    ],
                ),
                (
                    KeySpec::SpaceCadet(SpaceCadet::LeftCtrlOpenParen),
                    &[
                        "KC_LCPO",
                        "LCPO",
                        "space cadet left ctrl",
                        "ctrl (",
                        "ctrl paren",
                    ],
                ),
                (
                    KeySpec::SpaceCadet(SpaceCadet::RightCtrlCloseParen),
                    &[
                        "KC_RCPC",
                        "RCPC",
                        "space cadet right ctrl",
                        "ctrl )",
                        "ctrl paren",
                    ],
                ),
                (
                    KeySpec::SpaceCadet(SpaceCadet::LeftAltOpenParen),
                    &[
                        "KC_LAPO",
                        "LAPO",
                        "space cadet left alt",
                        "alt (",
                        "alt paren",
                    ],
                ),
                (
                    KeySpec::SpaceCadet(SpaceCadet::RightAltCloseParen),
                    &[
                        "KC_RAPC",
                        "RAPC",
                        "space cadet right alt",
                        "alt )",
                        "alt paren",
                    ],
                ),
                (
                    KeySpec::SpaceCadet(SpaceCadet::RightShiftEnter),
                    &[
                        "KC_SFTENT",
                        "SFTENT",
                        "space cadet shift enter",
                        "shift enter",
                    ],
                ),
            ],
        )
    })
}

fn qmk_auto_shift_group() -> &'static CandidateGroup {
    static GROUP: OnceLock<CandidateGroup> = OnceLock::new();
    GROUP.get_or_init(|| {
        build_qmk_group(
            "Auto Shift",
            &[
                (
                    KeySpec::AutoShift(AutoShiftAction::Down),
                    &[
                        "QK_AUTO_SHIFT_DOWN",
                        "AS_DOWN",
                        "auto shift down",
                        "autoshift down",
                    ],
                ),
                (
                    KeySpec::AutoShift(AutoShiftAction::Up),
                    &["QK_AUTO_SHIFT_UP", "AS_UP", "auto shift up", "autoshift up"],
                ),
                (
                    KeySpec::AutoShift(AutoShiftAction::Report),
                    &[
                        "QK_AUTO_SHIFT_REPORT",
                        "AS_RPT",
                        "auto shift report",
                        "autoshift report",
                    ],
                ),
                (
                    KeySpec::AutoShift(AutoShiftAction::On),
                    &["QK_AUTO_SHIFT_ON", "AS_ON", "auto shift on", "autoshift on"],
                ),
                (
                    KeySpec::AutoShift(AutoShiftAction::Off),
                    &[
                        "QK_AUTO_SHIFT_OFF",
                        "AS_OFF",
                        "auto shift off",
                        "autoshift off",
                    ],
                ),
                (
                    KeySpec::AutoShift(AutoShiftAction::Toggle),
                    &[
                        "QK_AUTO_SHIFT_TOGGLE",
                        "AS_TOGG",
                        "auto shift toggle",
                        "autoshift toggle",
                    ],
                ),
            ],
        )
    })
}

fn qmk_combo_group() -> &'static CandidateGroup {
    static GROUP: OnceLock<CandidateGroup> = OnceLock::new();
    GROUP.get_or_init(|| {
        build_qmk_group(
            "Combo Control",
            &[
                (
                    KeySpec::Combo(ComboAction::Toggle),
                    &["QK_COMBO_TOGGLE", "CM_TOGG", "combo toggle"],
                ),
                (
                    KeySpec::Combo(ComboAction::On),
                    &["QK_COMBO_ON", "CM_ON", "combo on"],
                ),
                (
                    KeySpec::Combo(ComboAction::Off),
                    &["QK_COMBO_OFF", "CM_OFF", "combo off"],
                ),
            ],
        )
    })
}

fn qmk_one_shot_control_group() -> &'static CandidateGroup {
    static GROUP: OnceLock<CandidateGroup> = OnceLock::new();
    GROUP.get_or_init(|| {
        build_qmk_group(
            "One-Shot Control",
            &[
                (
                    KeySpec::OneShot(OneShotAction::Toggle),
                    &[
                        "QK_ONE_SHOT_TOGGLE",
                        "OS_TOGG",
                        "oneshot toggle",
                        "one-shot toggle",
                    ],
                ),
                (
                    KeySpec::OneShot(OneShotAction::On),
                    &["QK_ONE_SHOT_ON", "OS_ON", "oneshot on", "one-shot on"],
                ),
                (
                    KeySpec::OneShot(OneShotAction::Off),
                    &["QK_ONE_SHOT_OFF", "OS_OFF", "oneshot off", "one-shot off"],
                ),
            ],
        )
    })
}

fn qmk_oneshot_mod_group() -> &'static CandidateGroup {
    static GROUP: OnceLock<CandidateGroup> = OnceLock::new();
    GROUP.get_or_init(|| {
        build_qmk_group(
            "One-Shot Mod",
            &[
                (
                    KeySpec::StickyKey {
                        key: None,
                        modifiers: Modifiers {
                            shift: true,
                            ..Default::default()
                        },
                    },
                    &[
                        "osm",
                        "osm_lsft",
                        "one-shot shift",
                        "oneshot shift",
                        "osm(mod_lsft)",
                    ],
                ),
                (
                    KeySpec::StickyKey {
                        key: None,
                        modifiers: Modifiers {
                            ctrl: true,
                            ..Default::default()
                        },
                    },
                    &[
                        "osm",
                        "osm_lctl",
                        "one-shot ctrl",
                        "oneshot ctrl",
                        "osm(mod_lctl)",
                    ],
                ),
                (
                    KeySpec::StickyKey {
                        key: None,
                        modifiers: Modifiers {
                            alt: true,
                            ..Default::default()
                        },
                    },
                    &[
                        "osm",
                        "osm_lalt",
                        "one-shot alt",
                        "oneshot alt",
                        "osm(mod_lalt)",
                    ],
                ),
                (
                    KeySpec::StickyKey {
                        key: None,
                        modifiers: Modifiers {
                            gui: true,
                            ..Default::default()
                        },
                    },
                    &[
                        "osm",
                        "osm_lgui",
                        "one-shot gui",
                        "oneshot gui",
                        "osm(mod_lgui)",
                    ],
                ),
                (
                    KeySpec::StickyKey {
                        key: None,
                        modifiers: Modifiers {
                            right_shift: true,
                            ..Default::default()
                        },
                    },
                    &[
                        "osm",
                        "osm_rsft",
                        "one-shot rshift",
                        "oneshot rshift",
                        "osm(mod_rsft)",
                    ],
                ),
                (
                    KeySpec::StickyKey {
                        key: None,
                        modifiers: Modifiers {
                            right_ctrl: true,
                            ..Default::default()
                        },
                    },
                    &[
                        "osm",
                        "osm_rctl",
                        "one-shot rctrl",
                        "oneshot rctrl",
                        "osm(mod_rctl)",
                    ],
                ),
                (
                    KeySpec::StickyKey {
                        key: None,
                        modifiers: Modifiers {
                            right_alt: true,
                            ..Default::default()
                        },
                    },
                    &[
                        "osm",
                        "osm_ralt",
                        "one-shot ralt",
                        "oneshot ralt",
                        "osm(mod_ralt)",
                    ],
                ),
                (
                    KeySpec::StickyKey {
                        key: None,
                        modifiers: Modifiers {
                            right_gui: true,
                            ..Default::default()
                        },
                    },
                    &[
                        "osm",
                        "osm_rgui",
                        "one-shot rgui",
                        "oneshot rgui",
                        "osm(mod_rgui)",
                    ],
                ),
            ],
        )
    })
}

fn qmk_special_groups() -> &'static [CandidateGroup] {
    static GROUPS: OnceLock<Vec<CandidateGroup>> = OnceLock::new();
    GROUPS.get_or_init(|| {
        let mut groups = vec![
            common::build_special_group(&QmkKeyPresenter, |spec| -> &'static [&'static str] {
                match spec {
                    KeySpec::Transparent => &["KC_TRNS"],
                    KeySpec::None => &["KC_NO"],
                    KeySpec::CapsWord => &["QK_CAPS_WORD_TOGGLE", "CW_TOGG"],
                    KeySpec::KeyRepeat => &["QK_KEY_REPEAT", "QK_REPEAT_KEY", "QK_REP"],
                    KeySpec::AltKeyRepeat => &["QK_ALT_REPEAT_KEY", "QK_AREP"],
                    KeySpec::KeyLock => &["QK_LOCK"],
                    KeySpec::GraveEscape => &["QK_GRAVE_ESCAPE", "QK_GESC"],
                    KeySpec::KeyPress { key, .. } => match key.id {
                        0x82 => &["KC_LOCKING_CAPS_LOCK", "KC_LCAP"],
                        0x83 => &["KC_LOCKING_NUM_LOCK", "KC_LNUM"],
                        0x84 => &["KC_LOCKING_SCROLL_LOCK", "KC_LSCR"],
                        0x7A => &["KC_UNDO"],
                        0x7B => &["KC_CUT"],
                        0x7C => &["KC_COPY"],
                        0x7D => &["KC_PASTE"],
                        0x7E => &["KC_FIND"],
                        0x79 => &["KC_AGAIN"],
                        0x78 => &["KC_STOP"],
                        0x66 => &["KC_POWER", "KC_PWR"],
                        0x68 => &["KC_F13"],
                        0x69 => &["KC_F14"],
                        0x6A => &["KC_F15"],
                        0x6B => &["KC_F16"],
                        0x6C => &["KC_F17"],
                        0x6D => &["KC_F18"],
                        0x6E => &["KC_F19"],
                        0x6F => &["KC_F20"],
                        0x70 => &["KC_F21"],
                        0x71 => &["KC_F22"],
                        0x72 => &["KC_F23"],
                        0x73 => &["KC_F24"],
                        _ => &[],
                    },
                    _ => &[],
                }
            }),
            qmk_space_cadet_group().clone(),
            qmk_auto_shift_group().clone(),
            qmk_combo_group().clone(),
            qmk_one_shot_control_group().clone(),
        ];
        groups.extend(qmk_extended_qmk_groups().iter().cloned());
        groups
    })
}

fn qmk_custom_groups() -> &'static [CandidateGroup] {
    static GROUPS: OnceLock<Vec<CandidateGroup>> = OnceLock::new();
    GROUPS.get_or_init(|| {
        let mut groups = common::build_custom_groups(&QmkKeyPresenter);
        let dm_keys = [
            (
                Keycode::QK_DYNAMIC_MACRO_RECORD_START_1 as u16,
                &["DM_REC1", "DM Rec 1", "dm record 1"][..],
            ),
            (
                Keycode::QK_DYNAMIC_MACRO_RECORD_START_2 as u16,
                &["DM_REC2", "DM Rec 2", "dm record 2"][..],
            ),
            (
                Keycode::QK_DYNAMIC_MACRO_RECORD_STOP as u16,
                &["DM_RSTP", "DM Stop", "dm stop"][..],
            ),
            (
                Keycode::QK_DYNAMIC_MACRO_PLAY_1 as u16,
                &["DM_PLY1", "DM Play 1", "dm play 1"][..],
            ),
            (
                Keycode::QK_DYNAMIC_MACRO_PLAY_2 as u16,
                &["DM_PLY2", "DM Play 2", "dm play 2"][..],
            ),
        ];

        let dm_candidates: Vec<Candidate> = dm_keys
            .into_iter()
            .map(|(code, tokens)| {
                let name = super::keycode_labels::try_resolve_qmk_key(code)
                    .map(|lk| lk.tap.full)
                    .unwrap_or_else(|| format!("0x{code:04X}"));
                let spec = KeySpec::Custom(CustomBinding {
                    kind: CustomKind::Raw,
                    id: code as u32,
                    name: Some(name),
                    param1: None,
                    param2: None,
                });
                Candidate::from_action(spec, &QmkKeyPresenter, &[])
                    .with_search_tokens(tokens.iter().copied())
            })
            .collect();

        groups.push(CandidateGroup {
            name: "Dynamic Macro",
            candidates: dm_candidates,
        });

        groups
    })
}

const QMK_LAYER_OPS: [(&str, LayerActivation, &[&str]); 7] = [
    (
        "Momentary",
        LayerActivation::Momentary,
        &["mo", "momentary"],
    ),
    ("Toggle", LayerActivation::Toggle, &["tg", "toggle"]),
    ("Switch To Layer", LayerActivation::To, &["to", "switch"]),
    (
        "One-Shot Layer",
        LayerActivation::Sticky,
        &["osl", "one-shot layer", "oneshot layer", "sl", "sticky"],
    ),
    (
        "Set Default Layer",
        LayerActivation::Default,
        &["df", "default"],
    ),
    (
        "Persistent Default Layer",
        LayerActivation::PersistentDefault,
        &["pdf", "persistent default", "default"],
    ),
    (
        "Tap Toggle",
        LayerActivation::TapToggle,
        &["tt", "tap toggle"],
    ),
];

impl KeyPresenter for QmkEditorProfile {
    fn present_key(&self, spec: &KeySpec, layer_names: &[String]) -> Option<LayoutKey> {
        QmkKeyPresenter.present_key(spec, layer_names)
    }
}

impl EditorProfile for QmkEditorProfile {
    fn presenter(&self) -> &dyn KeyPresenter {
        self
    }

    fn name(&self) -> &'static str {
        "QMK"
    }

    fn sidebar_sections(&self) -> &[SidebarSection<FeatureManifest>] {
        &QMK_SIDEBAR_SECTIONS
    }

    fn section_label(&self, section: EditorSection) -> &'static str {
        match section {
            EditorSection::OneShot => "One-Shot Mod",
            EditorSection::System => "System & Power",
            _ => section.label(),
        }
    }

    fn tap_categories(&self) -> &'static [CandidateGroup] {
        qmk_tap_categories()
    }

    fn section_groups(&self, section: EditorSection) -> &'static [CandidateGroup] {
        match section {
            EditorSection::Keyboard
            | EditorSection::Combo
            | EditorSection::ModTap
            | EditorSection::OneShot => qmk_tap_categories(),
            EditorSection::Media => qmk_media_groups(),
            EditorSection::Special => qmk_special_groups(),
            EditorSection::System => std::slice::from_ref(qmk_system_and_power_group()),
            EditorSection::Backlight => qmk_backlight_groups(),
            EditorSection::Rgb => qmk_rgb_groups(),
            EditorSection::RgbMatrix => qmk_rgb_matrix_groups(),
            EditorSection::Audio => qmk_audio_groups(),
            EditorSection::Mouse => qmk_mouse_groups(),
            EditorSection::Custom => qmk_custom_groups(),
            _ => &[],
        }
    }

    fn layer_groups(
        &self,
        layer_count: usize,
        layer_names: &[String],
        tap: LayerTapTarget,
    ) -> Vec<CandidateGroup> {
        common::build_layer_groups(
            &QmkKeyPresenter,
            layer_count,
            layer_names,
            tap,
            &QMK_LAYER_OPS,
            |_| &[],
        )
    }

    fn parse_raw_keycode(&self, raw: &str) -> Option<KeySpec> {
        let code = u16::from_str_radix(raw, 16).ok()?;
        Some(super::codec::qmk_to_keyspec(code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_catalog_candidates_encode() {
        let profile = QmkEditorProfile;
        let mut failures = Vec::new();
        let layer_names = Vec::new();
        let mut groups: Vec<CandidateGroup> = profile
            .tap_categories()
            .iter()
            .chain(
                profile
                    .sidebar_sections()
                    .iter()
                    .flat_map(|section| section.items)
                    .map(|item| item.section)
                    .flat_map(|section| profile.section_groups(section)),
            )
            .cloned()
            .collect();
        groups.extend(profile.layer_groups(2, &layer_names, LayerTapTarget::default()));

        for group in &groups {
            for candidate in &group.candidates {
                match super::super::codec::keyspec_to_qmk(&candidate.binding) {
                    Ok(code) if super::super::codec::qmk_to_keyspec(code) == candidate.binding => {}
                    _ => failures.push(format!("{}: {:?}", group.name, candidate.binding)),
                }
            }
        }

        assert!(
            failures.is_empty(),
            "catalog candidates failed to encode: {failures:?}"
        );
    }

    #[test]
    fn catalog_keeps_official_and_friendly_search_aliases() {
        let profile = QmkEditorProfile;
        let queries = [
            "QK_BOOTLOADER",
            "RM_ON",
            "BL_TOGG",
            "UG_TOGG",
            "UG_NEXT",
            "MS_WHLU",
            "QK_REPEAT_KEY",
            "QK_AREP",
            "QK_UNICODE_MODE_LINUX",
            "QK_AUTOCORRECT_ON",
            "QK_LEADER",
            "QK_SWAP_HANDS_TOGGLE",
            "QK_MAGIC_GUI_ON",
            "volume",
            "enter",
        ];

        for query in queries {
            assert!(
                profile
                    .tap_categories()
                    .iter()
                    .chain(
                        profile
                            .sidebar_sections()
                            .iter()
                            .flat_map(|section| section.items)
                            .map(|item| item.section)
                            .flat_map(|section| profile.section_groups(section)),
                    )
                    .flat_map(|group| &group.candidates)
                    .any(|candidate| candidate.matches_query(query)),
                "no QMK candidate matched {query:?}"
            );
        }

        assert!(profile
            .layer_groups(2, &[], LayerTapTarget::default())
            .iter()
            .flat_map(|group| &group.candidates)
            .any(|candidate| candidate.matches_query("layer tap")));
    }

    #[test]
    fn catalog_does_not_expose_zmk_only_features() {
        let profile = QmkEditorProfile;

        assert!(profile.section_groups(EditorSection::Bluetooth).is_empty());
        assert!(profile.section_groups(EditorSection::Output).is_empty());
        assert!(profile
            .section_groups(EditorSection::Backlight)
            .iter()
            .flat_map(|group| &group.candidates)
            .all(|candidate| !matches!(
                candidate.binding,
                KeySpec::Lighting(keypeek_core::LightingAction::Backlight(
                    BacklightAction::Set(_)
                ))
            )));
        assert!(profile
            .section_groups(EditorSection::Rgb)
            .iter()
            .flat_map(|group| &group.candidates)
            .all(|candidate| !matches!(
                candidate.binding,
                KeySpec::Lighting(keypeek_core::LightingAction::Rgb(
                    RgbAction::On | RgbAction::Off | RgbAction::EffectSet | RgbAction::Color
                ))
            )));
    }

    #[test]
    fn feature_manifest_matches_qmk_sidebar_and_catalog() {
        let profile = QmkEditorProfile;
        let mut seen = std::collections::HashSet::new();

        for feature in profile.feature_manifest() {
            assert!(
                seen.insert(feature.section),
                "duplicate section in sidebar: {:?}",
                feature.section
            );
            assert!(
                !profile.section_groups(feature.section).is_empty()
                    || matches!(
                        feature.section,
                        EditorSection::Layers
                            | EditorSection::Combo
                            | EditorSection::OneShot
                            | EditorSection::ModTap
                            | EditorSection::LayerMod
                            | EditorSection::RawHex
                    ),
                "feature {:?} has no catalog groups or dedicated page",
                feature.section
            );
        }
    }

    #[test]
    fn parameter_metadata_matches_qmk_bindings() {
        let profile = QmkEditorProfile;
        let layer_names = Vec::new();
        let mut groups: Vec<CandidateGroup> =
            profile.section_groups(EditorSection::Layers).to_vec();
        groups.extend(profile.layer_groups(2, &layer_names, LayerTapTarget::default()));

        for candidate in groups.iter().flat_map(|group| &group.candidates) {
            if let Some(parameter) = candidate.parameter {
                assert!(matches!(
                    (parameter, &candidate.binding),
                    (
                        keypeek_core::keymap_editor::ParameterSpec::Layer { .. },
                        KeySpec::Layer { .. } | KeySpec::LayerTap { .. }
                    )
                ));
            }
        }
    }
}
