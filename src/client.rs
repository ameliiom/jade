//! Client side: builds the tooltip for the crosshair's target and draws it.
use crate::payloads::{BLOCK_INFO, ENTITY_INFO};
use crate::settings::{self as s, Position, Role};
use crate::text::*;
use crate::{lock, MINING, TICKS};
use mod_api::{
    BlockState, ClientHost, CrosshairTarget, DrawContext, EntityRef, GuiDraw, Hand,
    HostBlockStates, Identifier, ItemKey, Text,
};
#[allow(unused_imports)]
use mod_api::prelude::*;
use mod_api::client::hud_layout::PoseOp;
use std::sync::atomic::Ordering;

/// The mining bar shows while the last mining prediction is at most this many ticks old.
const MINING_FRESH: u32 = 4;
/// Container contents shown at most: two rows of nine.
const MAX_ITEMS: usize = 18;
const ITEMS_PER_ROW: usize = 9;

/// Vanilla crops whose `age` property is growth. Any modded block with an `age` property also
/// counts; vanilla's other `age` blocks (fire, cactus, sugar cane, kelp, bamboo…) use it as an
/// internal counter.
const VANILLA_CROPS: &[&str] = &[
    "wheat",
    "carrots",
    "potatoes",
    "beetroots",
    "melon_stem",
    "pumpkin_stem",
    "cocoa",
    "nether_wart",
    "sweet_berry_bush",
    "torchflower_crop",
    "pitcher_crop",
];

/// What the overlay shows: an optional icon, lines of text, a row of items (container
/// contents, an item frame's item) and an optional progress bar (mining).
pub struct Tooltip {
    icon: Option<ItemKey>,
    lines: Vec<(String, Role)>,
    items: Vec<(ItemKey, i32)>,
    bar: Option<f32>,
}

/// The tooltip for what the crosshair is on, or None when it's on nothing.
pub fn look(host: &dyn ClientHost) -> Option<Tooltip> {
    match host.crosshair_target()? {
        CrosshairTarget::Block { pos, state, .. } => {
            Some(look_block(host, [pos.x, pos.y, pos.z], state))
        }
        CrosshairTarget::Entity { entity, .. } => look_entity(host, entity),
        _ => None,
    }
}

fn item_key(host: &dyn ClientHost, id: &str) -> Option<ItemKey> {
    host.item_key(&Identifier::parse(id).ok()?)
}

fn look_block(host: &dyn ClientHost, pos: [i32; 3], state: BlockState) -> Tooltip {
    let block = state.block();
    let id = block.id(host);
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", &id));
    let mut lines = vec![(id_name(host, "block", &id), Role::Title)];
    let mut items = Vec::new();
    let states = host.block_states();
    let text_of = |name: &str| -> Option<String> {
        Some(state.property(&states, name).ok()??.to_owned())
    };
    let number_of = |name: &str| -> Option<(u32, u32)> {
        let value: u32 = text_of(name)?.parse().ok()?;
        let max = states
            .definition(block)?
            .into_iter()
            .find(|p| p.name == name)?
            .values
            .iter()
            .filter_map(|v| v.parse::<u32>().ok())
            .max()?;
        Some((value, max))
    };
    let mut push = |key: &str, args: &[&str], role: Role| lines.push((tr(host, key, args), role));

    // Crop growth, as Jade's "Growth: 57%" / "Mature".
    if s::on(host, s::GROWTH)
        && (namespace != "minecraft" || VANILLA_CROPS.contains(&path))
        && let Some((age, max)) = number_of("age")
        && max > 0
    {
        if age >= max {
            push("jade.mature", &[], Role::Good);
        } else {
            push("jade.growth", &[&format!("{}%", age * 100 / max)], Role::Info);
        }
    }

    // Block state details: levels, redstone, note blocks…
    if s::on(host, s::BLOCK_STATES) {
        let level = |value: u32, max: u32| (format!("{value} / {max}"), value >= max);
        let mut lvl = |key: &str, prop: &str| {
            if let Some((value, max)) = number_of(prop) {
                let (text, full) = level(value, max);
                push(key, &[&text], if full { Role::Good } else { Role::Info });
            }
        };
        match path {
            "beehive" | "bee_nest" => lvl("jade.honey", "honey_level"),
            "composter" => lvl("jade.compost", "level"),
            "respawn_anchor" => lvl("jade.charges", "charges"),
            "farmland" => lvl("jade.moisture", "moisture"),
            "turtle_egg" | "sniffer_egg" => lvl("jade.hatch", "hatch"),
            "redstone_wire" | "daylight_detector" | "target" | "sculk_sensor"
            | "calibrated_sculk_sensor" | "weighted_pressure_plate" => {
                if let Some((power, _)) = number_of("power") {
                    push("jade.power", &[&power.to_string()], Role::Info);
                }
            }
            "repeater" => {
                if let Some(delay) = text_of("delay") {
                    push("jade.delay", &[&delay], Role::Info);
                }
            }
            "comparator" => {
                if let Some(mode) = text_of("mode") {
                    push(&format!("jade.comparator.{mode}"), &[], Role::Info);
                }
            }
            "lever" => {
                let on = text_of("powered").as_deref() == Some("true");
                push(
                    if on { "jade.on" } else { "jade.off" },
                    &[],
                    if on { Role::Good } else { Role::Bad },
                );
            }
            "note_block" => {
                if let Some((note, _)) = number_of("note") {
                    let instrument = text_of("instrument").map(|i| words(&i)).unwrap_or_default();
                    let text = format!("{} ({note}) \u{2013} {instrument}", note_name(note));
                    push("jade.note", &[&text], Role::Info);
                }
            }
            "cake" => {
                if let Some((bites, max)) = number_of("bites") {
                    let left = (max + 1 - bites).to_string();
                    push("jade.slices", &[&left, &(max + 1).to_string()], Role::Info);
                }
            }
            _ => {}
        }
    }

    // Server-side block entity data.
    if let Some(info) = lock(&BLOCK_INFO).as_ref().filter(|i| [i.x, i.y, i.z] == pos) {
        if s::on(host, s::WORK) {
            if let Some(f) = info.cooking {
                push("jade.cooking", &[&percent(f)], Role::Info);
            }
            if let Some(f) = info.brewing {
                push("jade.brewing", &[&percent(f)], Role::Info);
            }
        }
        if s::on(host, s::BLOCK_STATES) {
            if let Some(mob) = &info.spawner {
                push("jade.spawner", &[&id_name(host, "entity", mob)], Role::Info);
            }
            if let Some(bees) = info.bees {
                push("jade.bees", &[&bees.to_string()], Role::Info);
            }
            if let Some(record) = &info.record {
                let (ns, disc) = record.split_once(':').unwrap_or(("minecraft", record));
                let song = disc.strip_prefix("music_disc_").unwrap_or(disc);
                let name = translated(host, &format!("jukebox_song.{ns}.{song}"), song);
                push("jade.record", &[&name], Role::Info);
            }
        }
        if s::on(host, s::CONTAINERS) {
            if info.loot {
                push("jade.loot", &[], Role::Info);
            }
            items = info
                .items
                .iter()
                .filter_map(|i| Some((item_key(host, &i.id)?, i.count)))
                .take(MAX_ITEMS)
                .collect();
        }
    }

    // Harvestability, for blocks that drop nothing without the right tool.
    if s::on(host, s::HARVEST) && states.requires_correct_tool(state) == Some(true) {
        let held = host
            .local_player()
            .and_then(|p| host.living(p.entity()))
            .and_then(|l| l.hand_item(host, Hand::Main));
        let ok = held
            .as_ref()
            .and_then(|stack| states.is_correct_tool_for_drops(state, stack))
            .unwrap_or(false);
        if ok {
            push("jade.harvestable", &[], Role::Good);
        } else {
            push("jade.wrong_tool", &[], Role::Bad);
        }
    }
    if s::on(host, s::MOD_NAME) {
        lines.push((words(namespace), Role::Mod));
    }

    let now = TICKS.load(Ordering::Relaxed);
    let bar = lock(&MINING)
        .filter(|(at, progress, seen)| {
            *at == pos && *progress > 0.0 && now.wrapping_sub(*seen) <= MINING_FRESH
        })
        .map(|(_, progress, _)| progress.min(1.0))
        .filter(|_| s::on(host, s::MINING));

    Tooltip {
        icon: host.block_item(block),
        lines,
        items,
        bar,
    }
}

fn look_entity(host: &dyn ClientHost, entity: EntityRef) -> Option<Tooltip> {
    let name = entity.name(host).unwrap_or_default();
    let title = name.clone();
    let mut lines = vec![(name, Role::Title)];
    let mut items = Vec::new();
    let info = lock(&ENTITY_INFO);
    if let Some(info) = info.as_ref().filter(|i| i.id.0 as u32 == entity.id()) {
        let mut push =
            |key: &str, args: &[&str], role: Role| lines.push((tr(host, key, args), role));
        if s::on(host, s::HEALTH)
            && let (Some(health), Some(max)) = (info.health, info.max_health)
        {
            push("jade.health", &[&number(health), &number(max)], Role::Health);
        }
        if s::on(host, s::OWNER)
            && let Some(owner) = &info.owner
        {
            push("jade.owner", &[owner], Role::Info);
        }
        if s::on(host, s::MOUNTS) {
            if let Some(speed) = info.speed {
                push("jade.speed", &[&number(speed)], Role::Info);
            }
            if let Some(jump) = info.jump {
                push("jade.jump", &[&number(jump)], Role::Info);
            }
            if let Some(strength) = info.strength {
                push("jade.strength", &[&strength.to_string()], Role::Info);
            }
        }
        if s::on(host, s::ANIMALS) {
            match info.age {
                Some(age) if age < 0 => push("jade.grows_up", &[&duration(-age)], Role::Info),
                Some(age) if age > 0 => {
                    push("jade.breed_cooldown", &[&duration(age)], Role::Info)
                }
                _ => {}
            }
            if info.in_love.is_some() {
                push("jade.in_love", &[], Role::Love);
            }
            if let Some(egg) = info.egg.filter(|t| *t > 0) {
                push("jade.next_egg", &[&duration(egg)], Role::Info);
            }
        }
        if s::on(host, s::VILLAGERS) {
            if let Some(profession) = &info.profession {
                let (ns, job) = profession.split_once(':').unwrap_or(("minecraft", profession));
                let job = translated(host, &format!("entity.{ns}.villager.{job}"), job);
                let level = info
                    .villager_level
                    .map(|l| translated(host, &format!("merchant.level.{l}"), &l.to_string()))
                    .unwrap_or_default();
                // A villager with a job is already named after it: say the job once.
                if job == title {
                    push("jade.villager_level", &[&level], Role::Info);
                } else {
                    push("jade.villager", &[&job, &level], Role::Info);
                }
            }
            if let Some(time) = info.conversion {
                push("jade.curing", &[&duration(time)], Role::Good);
            }
        }
        if let Some(fuse) = info.fuse {
            push("jade.fuse", &[&number(fuse as f32 / 20.0)], Role::Bad);
        }
        if s::on(host, s::EFFECTS) {
            for effect in &info.effects {
                let name = id_name(host, "effect", &effect.id);
                let time = if effect.duration < 0 {
                    "\u{221E}".to_owned()
                } else {
                    duration(effect.duration)
                };
                let text = format!("{name} {} ({time})", roman(effect.amplifier));
                lines.push((text, Role::Info));
            }
        }
        if let Some(item) = &info.item
            && let Some(key) = item_key(host, &item.id)
        {
            items.push((key, item.count));
        }
        if s::on(host, s::MOD_NAME)
            && let Some((namespace, _)) = info.kind.split_once(':')
        {
            lines.push((words(namespace), Role::Mod));
        }
    }
    if lines.len() == 1 && lines[0].0.is_empty() && items.is_empty() {
        return None;
    }
    Some(Tooltip {
        icon: None,
        lines,
        items,
        bar: None,
    })
}

/// An item count as Jade shows it under an icon: 64, 1.2k.
fn count_text(count: i32) -> String {
    if count >= 1000 {
        format!("{}k", number(count as f32 / 1000.0))
    } else {
        count.to_string()
    }
}

impl Tooltip {
    pub fn draw(&self, host: &dyn ClientHost, draw: &mut dyn DrawContext) {
        const PAD: f32 = 4.0;
        const LINE: f32 = 10.0;
        const ICON: f32 = 16.0;
        const CELL: f32 = 18.0;
        const BAR: f32 = 2.0;
        const MARGIN: f32 = 4.0;
        let theme = s::theme(host);
        let scale = s::scale(host);

        let icon_room = if self.icon.is_some() { ICON + 4.0 } else { 0.0 };
        let text_w = self
            .lines
            .iter()
            .map(|(text, _)| draw.text_width(text))
            .fold(0.0, f32::max);
        let text_h = self.lines.len() as f32 * LINE - 1.0;
        let content_h = text_h.max(if self.icon.is_some() { ICON } else { 0.0 });
        let rows = self.items.len().div_ceil(ITEMS_PER_ROW);
        let items_w = self.items.len().min(ITEMS_PER_ROW) as f32 * CELL;
        let items_h = rows as f32 * CELL;
        let items_room = if rows > 0 { items_h + 3.0 } else { 0.0 };
        let bar_room = if self.bar.is_some() { BAR + 3.0 } else { 0.0 };
        let w = (icon_room + text_w).max(items_w) + 2.0 * PAD;
        let h = content_h + items_room + bar_room + 2.0 * PAD;

        let [screen_w, _] = draw.size();
        let x = match s::position(host) {
            Position::Center => ((screen_w - w * scale) / 2.0).round(),
            Position::Left => MARGIN,
            Position::Right => (screen_w - w * scale - MARGIN).round(),
        };
        let origin = [x, MARGIN];
        // Right-to-left languages mirror the box: icon on the right, lines right-aligned,
        // item rows and the bar filling from the right. `flip` maps an LTR box x to its place.
        let rtl = right_to_left(host);
        let flip = |x: f32, width: f32| if rtl { w - x - width } else { x };

        // The size setting goes through the GUI pose, which applies only inside one `gui_paint`
        // buffer: the frame, the lines and the bar go in one buffer, in box pixels, after a
        // `Push` that moves and scales them. Item icons can't go in a buffer (its `Item` op only
        // draws hotbar slots), so they are drawn on their own at scaled positions and sizes,
        // then a second buffer draws the stack counts over them.
        let mut paint = Painter::new(origin, scale);
        paint.fill([1.0, 0.0, w - 2.0, h], theme.background);
        paint.fill([0.0, 1.0, 1.0, h - 2.0], theme.background);
        paint.fill([w - 1.0, 1.0, 1.0, h - 2.0], theme.background);
        paint.gradient([1.0, 1.0, 1.0, h - 2.0], theme.border_top, theme.border_bottom);
        paint.gradient([w - 2.0, 1.0, 1.0, h - 2.0], theme.border_top, theme.border_bottom);
        paint.fill([1.0, 1.0, w - 2.0, 1.0], theme.border_top);
        paint.fill([1.0, h - 2.0, w - 2.0, 1.0], theme.border_bottom);

        let text_x = PAD + icon_room;
        let text_y = PAD + ((content_h - text_h) / 2.0).round();
        for (i, (text, role)) in self.lines.iter().enumerate() {
            let at = [flip(text_x, draw.text_width(text)), text_y + i as f32 * LINE];
            paint.text(text, at, theme.colour(*role), theme.shadow);
        }

        let items_y = PAD + content_h + 3.0;
        if let Some(progress) = self.bar {
            let bar_y = items_y + items_room;
            let bar_w = w - 2.0 * PAD;
            paint.fill([PAD, bar_y, bar_w, BAR], theme.bar_back);
            let fill_w = (bar_w * progress).round();
            paint.fill([flip(PAD, fill_w), bar_y, fill_w, BAR], theme.bar_fill);
        }
        paint.finish(draw);

        // Icons: box pixels → screen pixels by hand.
        let on_screen = |[bx, by]: [f32; 2]| [origin[0] + bx * scale, origin[1] + by * scale];
        if let Some(icon) = self.icon {
            let icon_y = PAD + ((content_h - ICON) / 2.0).round();
            draw.item(icon, on_screen([flip(PAD, ICON), icon_y]), ICON * scale);
        }
        let cell = |i: usize| {
            let cx = PAD + (i % ITEMS_PER_ROW) as f32 * CELL + 1.0;
            let cy = items_y + (i / ITEMS_PER_ROW) as f32 * CELL + 1.0;
            [flip(cx, ICON), cy]
        };
        for (i, (key, _)) in self.items.iter().enumerate() {
            draw.item(*key, on_screen(cell(i)), ICON * scale);
        }
        // Counts over the icons, bottom-right as vanilla's.
        if self.items.iter().any(|(_, count)| *count > 1) {
            let mut counts = Painter::new(origin, scale);
            for (i, (_, count)) in self.items.iter().enumerate() {
                if *count > 1 {
                    let [cx, cy] = cell(i);
                    let label = count_text(*count);
                    let lw = draw.text_width(&label);
                    counts.text(&label, [cx + ICON + 1.0 - lw, cy + 9.0], 0xFFFF_FFFF, true);
                }
            }
            counts.finish(draw);
        }
    }
}

/// Whether the player's language reads right to left. The game gives no direction flag, so
/// this asks the language's own name (`language.name`, "العربية") and takes its first strong
/// letter's direction, as Unicode's paragraph rule does: no list of languages to keep.
fn right_to_left(host: &dyn ClientHost) -> bool {
    host.translate("language.name").is_some_and(|name| starts_rtl(&name))
}

/// The direction of the first letter: true for the right-to-left scripts' blocks.
fn starts_rtl(text: &str) -> bool {
    text.chars().find(|c| c.is_alphabetic()).is_some_and(|c| {
        matches!(c as u32,
            0x0590..=0x08FF      // Hebrew, Arabic, Syriac, Thaana, NKo, Samaritan, Mandaic
            | 0xFB1D..=0xFDFF    // Hebrew and Arabic presentation forms A
            | 0xFE70..=0xFEFF    // Arabic presentation forms B
            | 0x10800..=0x10FFF  // historic right-to-left scripts
            | 0x1E800..=0x1EFFF) // Mende Kikakui, Adlam, Arabic mathematical
    })
}

/// One `gui_paint` buffer drawn in box pixels: it opens with a pose that moves the box to
/// `origin` (screen pixels) and scales it, and closes the pose at the end.
struct Painter {
    ops: Vec<GuiDraw>,
}

impl Painter {
    fn new(origin: [f32; 2], scale: f32) -> Self {
        Self {
            ops: vec![GuiDraw::Push(vec![
                PoseOp::Translate(origin[0], origin[1]),
                PoseOp::Scale(scale, scale),
            ])],
        }
    }
    /// `[x, y, w, h]` → the op's sorted corners.
    fn corners([x, y, w, h]: [f32; 4]) -> [i32; 4] {
        let r = |v: f32| v.round() as i32;
        [r(x), r(y), r(x + w), r(y + h)]
    }
    fn fill(&mut self, rect: [f32; 4], color: u32) {
        let [x0, y0, x1, y1] = Self::corners(rect);
        self.ops.push(GuiDraw::Fill { x0, y0, x1, y1, color });
    }
    fn gradient(&mut self, rect: [f32; 4], top: u32, bottom: u32) {
        let [x0, y0, x1, y1] = Self::corners(rect);
        self.ops.push(GuiDraw::FillGradient { x0, y0, x1, y1, top, bottom });
    }
    fn text(&mut self, text: &str, [x, y]: [f32; 2], color: u32, shadow: bool) {
        self.ops.push(GuiDraw::Text {
            text: Text::literal(text),
            x: x.round() as i32,
            y: y.round() as i32,
            color,
            shadow,
        });
    }
    fn finish(mut self, draw: &mut dyn DrawContext) {
        self.ops.push(GuiDraw::Pop);
        // A refusal (text too deep) would only drop this frame's tooltip.
        let _ = draw.gui_paint(&self.ops);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_follows_the_first_letter() {
        assert!(starts_rtl("العربية"));
        assert!(starts_rtl("עברית"));
        assert!(starts_rtl("فارسی"));
        assert!(!starts_rtl("English"));
        assert!(!starts_rtl("Français"));
        assert!(!starts_rtl("日本語"));
        assert!(!starts_rtl(""));
    }

    #[test]
    fn counts_shorten_past_a_thousand() {
        assert_eq!(count_text(64), "64");
        assert_eq!(count_text(1234), "1.2k");
        assert_eq!(count_text(2000), "2k");
    }
}
