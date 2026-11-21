//! Server side: answers the client's questions from data only the server has. Every answer is
//! limited to targets near the asking player, so the payloads can't spy on far-away places.
use crate::payloads::*;
use mod_api::{Compound, Host, HostBlockStates, IPayloadContext, PlayerRef, Tag};
#[allow(unused_imports)]
use mod_api::prelude::*;
use std::collections::BTreeMap;

/// The farthest target the server answers about, in blocks from the player's eyes.
const MAX_DISTANCE: f64 = 24.0;
/// A brewing stand's full brew, in ticks (`BrewingStandBlockEntity`).
const BREW_TICKS: f32 = 400.0;
/// `movement_speed` attribute → blocks per second, as horse stat mods compute it.
const SPEED_TO_BLOCKS: f64 = 42.157_796_4;

fn near(read: &dyn Host, player: PlayerRef, at: [f64; 3]) -> bool {
    let Some(eye) = player.entity().eye_position(read) else {
        return false;
    };
    let distance_sqr: f64 = (0..3).map(|i| (at[i] - eye[i]).powi(2)).sum();
    distance_sqr <= MAX_DISTANCE * MAX_DISTANCE
}

// --- NBT reads -------------------------------------------------------------------------------

fn get<'a>(data: &'a Compound, key: &str) -> Option<&'a Tag> {
    data.get(&key.into())
}

fn int(data: &Compound, key: &str) -> Option<i32> {
    match get(data, key)? {
        Tag::Int(v) => Some(*v),
        Tag::Short(v) => Some(*v as i32),
        Tag::Byte(v) => Some(*v as i32),
        Tag::Long(v) => Some(*v as i32),
        _ => None,
    }
}

fn double(tag: &Tag) -> Option<f64> {
    match tag {
        Tag::Double(bits) => Some(f64::from_bits(*bits)),
        Tag::Float(bits) => Some(f32::from_bits(*bits) as f64),
        Tag::Int(v) => Some(*v as f64),
        _ => None,
    }
}

fn string(data: &Compound, key: &str) -> Option<String> {
    match get(data, key)? {
        Tag::String(s) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn compound<'a>(data: &'a Compound, key: &str) -> Option<&'a Compound> {
    match get(data, key)? {
        Tag::Compound(c) => Some(c),
        _ => None,
    }
}

fn list<'a>(data: &'a Compound, key: &str) -> &'a [Tag] {
    match get(data, key) {
        Some(Tag::List(items)) => items,
        _ => &[],
    }
}

fn compounds<'a>(data: &'a Compound, key: &str) -> impl Iterator<Item = &'a Compound> {
    list(data, key).iter().filter_map(|t| match t {
        Tag::Compound(c) => Some(c),
        _ => None,
    })
}

/// An item stack's compound (`{id, count}`; `Count` before 1.20.5).
fn item(data: &Compound) -> Option<ItemCount> {
    let id = string(data, "id")?;
    let count = int(data, "count").or_else(|| int(data, "Count")).unwrap_or(1);
    Some(ItemCount { id, count })
}

/// A UUID stored as four ints.
fn uuid(data: &Compound, key: &str) -> Option<u128> {
    match get(data, key)? {
        Tag::IntArray(v) if v.len() == 4 => Some(
            v.iter()
                .fold(0u128, |acc, part| (acc << 32) | (*part as u32 as u128)),
        ),
        _ => None,
    }
}

/// A base attribute value from the entity's `attributes` list.
fn attribute(data: &Compound, id: &str) -> Option<f64> {
    compounds(data, "attributes")
        .find(|a| string(a, "id").as_deref() == Some(id))
        .and_then(|a| get(a, "base"))
        .and_then(double)
}

/// A horse's jump height in blocks from its `jump_strength` (the usual fitted curve).
fn jump_height(strength: f64) -> f64 {
    -0.181_758_495_2 * strength.powi(3) + 3.689_713_992 * strength.powi(2)
        + 2.128_599_134 * strength
        - 0.343_930_367
}

fn uuid_text(uuid: u128) -> String {
    let h = format!("{uuid:032x}");
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

/// The other half of a double chest (`ChestBlock.getConnectedDirection`): a `left` half's
/// partner is clockwise of its `facing`, a `right` half's counter-clockwise. None for a single
/// chest or a block without these properties.
fn connected_half(pos: [i32; 3], facing: &str, kind: &str) -> Option<[i32; 3]> {
    // Clockwise seen from above: north, east, south, west.
    const AROUND: [(&str, [i32; 2]); 4] = [
        ("north", [0, -1]),
        ("east", [1, 0]),
        ("south", [0, 1]),
        ("west", [-1, 0]),
    ];
    let at = AROUND.iter().position(|(name, _)| *name == facing)?;
    let turn = match kind {
        "left" => 1,
        "right" => 3,
        _ => return None,
    };
    let [dx, dz] = AROUND[(at + turn) % 4].1;
    Some([pos[0] + dx, pos[1], pos[2] + dz])
}

/// A container's contents merged by item, most first; nothing for a locked container.
fn contents(data: &Compound, locked: bool) -> Vec<ItemCount> {
    if locked {
        return Vec::new();
    }
    let mut merged: BTreeMap<String, i32> = BTreeMap::new();
    for stack in compounds(data, "Items").filter_map(item) {
        *merged.entry(stack.id).or_default() += stack.count;
    }
    let mut items: Vec<ItemCount> = merged
        .into_iter()
        .map(|(id, count)| ItemCount { id, count })
        .collect();
    items.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
    items
}

// --- Answers ---------------------------------------------------------------------------------

/// `jade:peek_entity`.
pub fn answer_entity(value: PeekEntity, cx: &mut dyn IPayloadContext) -> Result<(), String> {
    let player = cx.player().map_err(|e| format!("{e:?}"))?;
    let host = cx.server_host().map_err(|e| format!("{e:?}"))?;
    let Some(entity) = host.entity(value.id.0 as u32) else {
        return Ok(());
    };
    let (kind, health, max_health) = {
        let read: &dyn Host = &*host;
        match entity.position(read) {
            Some(at) if near(read, player, at) => {}
            _ => return Ok(()),
        }
        let kind = entity.kind(read).map(|key| key.id(read)).unwrap_or_default();
        match host.living(entity) {
            Some(l) => (kind, l.health(read), l.max_health(read)),
            None => (kind, None, None),
        }
    };
    let data = entity.nbt(host).unwrap_or_default();
    let read: &dyn Host = &*host;

    // A tadpole's `Age` counts up to becoming a frog, not the ageable-mob convention; an
    // item entity's `Age` is its despawn timer.
    let ageable = health.is_some() && kind != "minecraft:tadpole";
    let owner = uuid(&data, "Owner").map(|id| {
        host.player(id)
            .and_then(|p| p.name(read))
            .unwrap_or_else(|| uuid_text(id))
    });
    let jump = attribute(&data, "minecraft:jump_strength");
    let villager = compound(&data, "VillagerData");
    let effects = compounds(&data, "active_effects")
        .filter_map(|e| {
            Some(EffectInfo {
                id: string(e, "id")?,
                amplifier: int(e, "amplifier").unwrap_or(0),
                duration: int(e, "duration").unwrap_or(0),
            })
        })
        .collect();
    let reply = EntityInfo {
        id: value.id,
        health,
        max_health,
        age: int(&data, "Age").filter(|_| ageable),
        in_love: int(&data, "InLove").filter(|t| *t > 0),
        egg: int(&data, "EggLayTime").filter(|_| kind == "minecraft:chicken"),
        owner,
        speed: jump
            .and(attribute(&data, "minecraft:movement_speed"))
            .map(|s| (s * SPEED_TO_BLOCKS) as f32),
        jump: jump.map(|s| jump_height(s) as f32),
        strength: int(&data, "Strength"),
        profession: villager
            .and_then(|v| string(v, "profession"))
            .filter(|p| p != "minecraft:none"),
        villager_level: villager.and_then(|v| int(v, "level")),
        conversion: int(&data, "ConversionTime").filter(|t| *t > 0),
        fuse: int(&data, "fuse").filter(|_| kind == "minecraft:tnt"),
        effects,
        item: compound(&data, "Item").and_then(item),
        kind,
    };
    ANSWER_ENTITY
        .get()
        .ok_or("jade: entity reply channel not registered")?
        .reply(cx, &reply)
        .map(drop)
        .map_err(|e| format!("{e:?}"))
}

/// `jade:peek_block`.
pub fn answer_block(value: PeekBlock, cx: &mut dyn IPayloadContext) -> Result<(), String> {
    let player = cx.player().map_err(|e| format!("{e:?}"))?;
    let host = cx.server_host().map_err(|e| format!("{e:?}"))?;
    let read: &dyn Host = &*host;
    let pos = [value.x, value.y, value.z];
    if !near(read, player, pos.map(|c| c as f64 + 0.5)) {
        return Ok(());
    }
    let Some(level) = player.entity().level(read) else {
        return Ok(());
    };
    let Some(block_entity) = host.block_entity(level, pos) else {
        return Ok(());
    };
    let Some(data) = block_entity.data(read) else {
        return Ok(());
    };
    // A locked container (vanilla `lock`, on this block or on the other half of a double
    // chest) refuses to open without its key: its contents stay hidden from everyone, as
    // Jade does. Whether the player holds the key isn't checked, so the key holder doesn't
    // see them either.
    let other_half = level.block_state(read, pos).and_then(|state| {
        let states = read.block_states();
        let facing = state.property(&states, "facing").ok()??;
        let kind = state.property(&states, "type").ok()??;
        connected_half(pos, facing, kind)
    });
    let locked = block_entity.lock(read).is_some()
        || other_half
            .and_then(|other| host.block_entity(level, other))
            .is_some_and(|other| other.lock(read).is_some());

    // Furnace, smoker, blast furnace: names of 1.21.2+, then the older ones.
    let spent = int(&data, "cooking_time_spent").or_else(|| int(&data, "CookTime"));
    let total = int(&data, "cooking_total_time").or_else(|| int(&data, "CookTimeTotal"));
    let cooking = match (spent, total) {
        (Some(spent), Some(total)) if total > 0 && spent > 0 => Some(spent as f32 / total as f32),
        _ => None,
    };
    // Brewing stand: `BrewTime` counts down from 400 while it brews, 0 when idle.
    let brewing = int(&data, "BrewTime")
        .filter(|t| *t > 0)
        .map(|t| 1.0 - t as f32 / BREW_TICKS);

    let reply = BlockInfo {
        x: value.x,
        y: value.y,
        z: value.z,
        cooking,
        brewing,
        items: contents(&data, locked),
        loot: !locked && get(&data, "LootTable").is_some(),
        spawner: compound(&data, "SpawnData")
            .and_then(|s| compound(s, "entity"))
            .and_then(|e| string(e, "id")),
        bees: get(&data, "bees").map(|_| list(&data, "bees").len() as i32),
        record: compound(&data, "RecordItem")
            .and_then(item)
            .map(|i| i.id),
    };
    ANSWER_BLOCK
        .get()
        .ok_or("jade: block reply channel not registered")?
        .reply(cx, &reply)
        .map(drop)
        .map_err(|e| format!("{e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jump_height_matches_known_values() {
        // Strength 0.4 to 1.0 gives 1.1 to 5.3 blocks; the middle 0.7, about 2.9.
        assert!((jump_height(0.4) - 1.1).abs() < 0.1);
        assert!((jump_height(0.7) - 2.89).abs() < 0.05);
        assert!((jump_height(1.0) - 5.29).abs() < 0.05);
    }

    fn chest_with(items: &[(&str, i32)]) -> Compound {
        let stacks = items
            .iter()
            .map(|(id, count)| {
                let mut stack = Compound::new();
                stack.insert("id".into(), Tag::String((*id).into()));
                stack.insert("count".into(), Tag::Int(*count));
                Tag::Compound(stack)
            })
            .collect();
        let mut data = Compound::new();
        data.insert("Items".into(), Tag::List(stacks));
        data
    }

    #[test]
    fn an_unlocked_chest_shows_its_contents_merged() {
        let data = chest_with(&[("minecraft:dirt", 10), ("minecraft:diamond", 3), ("minecraft:dirt", 5)]);
        let items = contents(&data, false);
        assert_eq!(items.len(), 2);
        assert_eq!((items[0].id.as_str(), items[0].count), ("minecraft:dirt", 15));
        assert_eq!((items[1].id.as_str(), items[1].count), ("minecraft:diamond", 3));
    }

    #[test]
    fn a_locked_chest_shows_nothing() {
        let data = chest_with(&[("minecraft:diamond", 64)]);
        assert!(contents(&data, true).is_empty());
    }

    #[test]
    fn a_double_chest_finds_its_other_half() {
        // Facing north: the left half's partner is east, the right half's west.
        assert_eq!(connected_half([0, 64, 0], "north", "left"), Some([1, 64, 0]));
        assert_eq!(connected_half([0, 64, 0], "north", "right"), Some([-1, 64, 0]));
        assert_eq!(connected_half([0, 64, 0], "east", "left"), Some([0, 64, 1]));
        assert_eq!(connected_half([0, 64, 0], "south", "left"), Some([-1, 64, 0]));
        assert_eq!(connected_half([0, 64, 0], "west", "right"), Some([0, 64, 1]));
        // Each half points at the other.
        for facing in ["north", "east", "south", "west"] {
            let other = connected_half([5, 64, 5], facing, "left").unwrap();
            assert_eq!(connected_half(other, facing, "right"), Some([5, 64, 5]));
        }
        assert_eq!(connected_half([0, 64, 0], "north", "single"), None);
        assert_eq!(connected_half([0, 64, 0], "up", "left"), None);
    }

    #[test]
    fn uuid_reads_four_ints() {
        let mut data = Compound::new();
        data.insert("Owner".into(), Tag::IntArray(vec![0, 0, 0, 1]));
        assert_eq!(uuid(&data, "Owner"), Some(1));
        assert_eq!(uuid_text(1), "00000000-0000-0000-0000-000000000001");
    }
}
