//! The two client → server questions and their answers, and the client's copy of the last
//! answers. Registered (version "4", optional: 4 added villager trades) in `Hooks::register`.
use mod_api::{PayloadCodec, ToClient, ToServer, VarInt};
use std::sync::{Mutex, OnceLock};

/// "Tell me about entity `id`."
#[derive(PayloadCodec)]
pub struct PeekEntity {
    pub id: VarInt,
}

/// An item and how many of it.
#[derive(PayloadCodec, Clone)]
pub struct ItemCount {
    pub id: String,
    pub count: i32,
}

/// A mob effect: id (`minecraft:speed`), level from 0, ticks left (-1: infinite).
#[derive(PayloadCodec, Clone)]
pub struct EffectInfo {
    pub id: String,
    pub amplifier: i32,
    pub duration: i32,
}

/// A merchant's trade: what it costs (one or two items), what it gives, and whether it is out
/// of stock.
#[derive(PayloadCodec, Clone)]
pub struct TradeInfo {
    pub costs: Vec<ItemCount>,
    pub result: ItemCount,
    pub out: bool,
}

/// What the server knows about entity `id`. Every field but `id` and `kind` is optional: each
/// is sent only for the entities that have it.
#[derive(PayloadCodec)]
pub struct EntityInfo {
    pub id: VarInt,
    /// The entity type's id (`minecraft:zombie`): gives the mod it comes from.
    pub kind: String,
    pub health: Option<f32>,
    pub max_health: Option<f32>,
    /// `AgeableMob.age`: below 0, ticks until a baby grows up; above 0, breeding cooldown.
    pub age: Option<i32>,
    /// `Animal.inLove`: ticks left in love mode.
    pub in_love: Option<i32>,
    /// `Chicken.eggTime`: ticks until the next egg.
    pub egg: Option<i32>,
    /// A tamed animal's owner, by name when the server knows it, else by UUID.
    pub owner: Option<String>,
    /// Horse-like mounts: speed in blocks per second, jump height in blocks.
    pub speed: Option<f32>,
    pub jump: Option<f32>,
    /// Llama strength (chest slots / 3).
    pub strength: Option<i32>,
    /// Villager profession id (`minecraft:farmer`) and level (1 to 5).
    pub profession: Option<String>,
    pub villager_level: Option<i32>,
    /// Zombie villager: ticks until cured.
    pub conversion: Option<i32>,
    /// Primed TNT: ticks until it explodes.
    pub fuse: Option<i32>,
    pub effects: Vec<EffectInfo>,
    /// Item frame: the item it holds.
    pub item: Option<ItemCount>,
    /// A villager's or wandering trader's first `MAX_TRADES` trades, in the trading screen's order.
    pub trades: Vec<TradeInfo>,
}

/// Trades shown at most: the first six of the trading screen's list.
pub const MAX_TRADES: usize = 6;

/// The first `MAX_TRADES` of a merchant's offers, in their order.
pub fn capped<T>(offers: impl IntoIterator<Item = T>) -> Vec<T> {
    offers.into_iter().take(MAX_TRADES).collect()
}

/// "Tell me about the block entity at this position."
#[derive(PayloadCodec)]
pub struct PeekBlock {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// What the server knows about the block entity at a position.
#[derive(PayloadCodec)]
pub struct BlockInfo {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Furnace cooking and brewing stand progress, from 0 to 1.
    pub cooking: Option<f32>,
    pub brewing: Option<f32>,
    /// A container's items, merged by item (chest, barrel, furnace, hopper, campfire…).
    pub items: Vec<ItemCount>,
    /// A container that hasn't generated its loot yet.
    pub loot: bool,
    /// A spawner's mob type id.
    pub spawner: Option<String>,
    /// Bees inside a hive or nest.
    pub bees: Option<i32>,
    /// A jukebox's disc.
    pub record: Option<String>,
}

pub static ASK_ENTITY: OnceLock<ToServer<PeekEntity>> = OnceLock::new();
pub static ANSWER_ENTITY: OnceLock<ToClient<EntityInfo>> = OnceLock::new();
pub static ASK_BLOCK: OnceLock<ToServer<PeekBlock>> = OnceLock::new();
pub static ANSWER_BLOCK: OnceLock<ToClient<BlockInfo>> = OnceLock::new();

/// The server's last answers on the client, each kept for the target it was about.
pub static ENTITY_INFO: Mutex<Option<EntityInfo>> = Mutex::new(None);
pub static BLOCK_INFO: Mutex<Option<BlockInfo>> = Mutex::new(None);
