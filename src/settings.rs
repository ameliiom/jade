//! The mod's options, shown in Options > Mods > Jade for Horizon > Config (Horizon builds the
//! screen from these specs), and the theme palettes.
use mod_api::{ClientHost, SettingSpec};

/// A line toggle: setting key, on by default.
pub const MOD_NAME: &str = "show_mod_name";
pub const HEALTH: &str = "show_health";
pub const GROWTH: &str = "show_growth";
pub const MINING: &str = "show_mining";
pub const HARVEST: &str = "show_harvest";
pub const BLOCK_STATES: &str = "show_block_states";
pub const WORK: &str = "show_work";
pub const CONTAINERS: &str = "show_containers";
pub const ANIMALS: &str = "show_animals";
pub const MOUNTS: &str = "show_mounts";
pub const OWNER: &str = "show_owner";
pub const VILLAGERS: &str = "show_villagers";
pub const TRADES: &str = "show_trades";
pub const EFFECTS: &str = "show_effects";

const TOGGLES: &[&str] = &[
    MOD_NAME,
    HEALTH,
    GROWTH,
    MINING,
    HARVEST,
    BLOCK_STATES,
    WORK,
    CONTAINERS,
    ANIMALS,
    MOUNTS,
    OWNER,
    VILLAGERS,
    TRADES,
    EFFECTS,
];

const THEME: &str = "theme";
const POSITION: &str = "position";
const SCALE: &str = "scale";

pub fn specs() -> Vec<SettingSpec> {
    let mut specs = vec![
        SettingSpec::choice(
            THEME,
            "jade.config.theme",
            ["vanilla", "dark", "light", "waila"].map(|t| format!("jade.config.theme.{t}")),
            0,
        ),
        SettingSpec::choice(
            POSITION,
            "jade.config.position",
            ["top_center", "top_left", "top_right"]
                .map(|p| format!("jade.config.position.{p}")),
            0,
        ),
        SettingSpec::range(SCALE, "jade.config.scale", 0.5, 2.0, 0.1, 1.0),
    ];
    for key in TOGGLES {
        specs.push(SettingSpec::toggle(*key, format!("jade.config.{key}"), true));
    }
    specs
}

/// Whether a line toggle is on (on when the setting can't be read).
pub fn on(host: &dyn ClientHost, key: &str) -> bool {
    host.setting_bool(key).unwrap_or(true)
}

pub fn scale(host: &dyn ClientHost) -> f32 {
    host.setting_f64(SCALE).unwrap_or(1.0).clamp(0.5, 2.0) as f32
}

/// Where the box goes along the top of the screen.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Center,
    Left,
    Right,
}

pub fn position(host: &dyn ClientHost) -> Position {
    match host.setting_choice(POSITION) {
        Some(1) => Position::Left,
        Some(2) => Position::Right,
        _ => Position::Center,
    }
}

/// What a line is, so each theme can colour it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Title,
    Info,
    Good,
    Bad,
    Love,
    Health,
    Mod,
}

/// A theme's colours, ARGB.
pub struct Theme {
    pub background: u32,
    pub border_top: u32,
    pub border_bottom: u32,
    pub bar_back: u32,
    pub bar_fill: u32,
    title: u32,
    info: u32,
    good: u32,
    bad: u32,
    love: u32,
    health: u32,
    module: u32,
    pub shadow: bool,
}

impl Theme {
    pub fn colour(&self, role: Role) -> u32 {
        match role {
            Role::Title => self.title,
            Role::Info => self.info,
            Role::Good => self.good,
            Role::Bad => self.bad,
            Role::Love => self.love,
            Role::Health => self.health,
            Role::Mod => self.module,
        }
    }
}

/// Vanilla's tooltip, which Jade's default theme copies.
const VANILLA: Theme = Theme {
    background: 0xF010_0010,
    border_top: 0x5050_00FF,
    border_bottom: 0x5028_007F,
    bar_back: 0xFF40_4040,
    bar_fill: 0xFFFF_FFFF,
    title: 0xFFFF_FFFF,
    info: 0xFFAA_AAAA,
    good: 0xFF55_FF55,
    bad: 0xFFFF_5555,
    love: 0xFFFF_77CC,
    health: 0xFFFF_5555,
    module: 0xFF55_55FF,
    shadow: true,
};

const DARK: Theme = Theme {
    background: 0xE01A_1A1A,
    border_top: 0xFF3A_3A3A,
    border_bottom: 0xFF2A_2A2A,
    bar_back: 0xFF30_3030,
    bar_fill: 0xFFE0_E0E0,
    title: 0xFFF0_F0F0,
    info: 0xFFA0_A0A0,
    good: 0xFF6B_D96B,
    bad: 0xFFE0_6060,
    love: 0xFFF0_80C0,
    health: 0xFFE0_6060,
    module: 0xFF7A_9CFF,
    shadow: false,
};

const LIGHT: Theme = Theme {
    background: 0xF0F4_F4F4,
    border_top: 0xFFC8_C8C8,
    border_bottom: 0xFFB0_B0B0,
    bar_back: 0xFFD0_D0D0,
    bar_fill: 0xFF40_4040,
    title: 0xFF20_2020,
    info: 0xFF60_6060,
    good: 0xFF1E_8C1E,
    bad: 0xFFC0_2020,
    love: 0xFFC0_3080,
    health: 0xFFC0_2020,
    module: 0xFF20_40C0,
    shadow: false,
};

/// The look of Waila, Jade's ancestor: black box, grey border, classic colours.
const WAILA: Theme = Theme {
    background: 0xCC00_0000,
    border_top: 0xCC50_5050,
    border_bottom: 0xCC50_5050,
    bar_back: 0xFF20_2020,
    bar_fill: 0xFFFF_FFFF,
    title: 0xFFFF_FFFF,
    info: 0xFFAA_AAAA,
    good: 0xFF55_FF55,
    bad: 0xFFFF_5555,
    love: 0xFFFF_55FF,
    health: 0xFFFF_5555,
    module: 0xFF55_55FF,
    shadow: true,
};

pub fn theme(host: &dyn ClientHost) -> &'static Theme {
    match host.setting_choice(THEME) {
        Some(1) => &DARK,
        Some(2) => &LIGHT,
        Some(3) => &WAILA,
        _ => &VANILLA,
    }
}
