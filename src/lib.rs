//! Jade: shows what the crosshair is on, at the top of the screen.
//!
//! An unofficial port of Snownee's Jade (NeoForge) to Horizon's WebAssembly mod API. Jade's Java
//! code can't run in Horizon, so this is a rewrite of its features on Horizon's hooks.
//!
//! What the client already knows is read on the client (`client.rs`): block state properties,
//! the held item, the client's own mining prediction. The rest lives only on the server (other
//! mobs' health, entity data, block entity data), so the client asks for it a few times a second
//! with two optional payloads (`payloads.rs`, answered in `server.rs`), the way Jade's server
//! data providers do. On a server without this mod those lines are simply missing.
//!
//! Options (theme, position, size, which lines show) are Horizon mod settings
//! (`settings.rs`), edited in Options > Mods > Jade > Config.
mod client;
mod payloads;
mod server;
mod settings;
mod text;

use mod_api::{
    ClientHost, ClientMiningPredictEvent, ClientTickPreEvent, CrosshairTarget, Flow, GuiLayer,
    GuiLayerDrawEvent, GuiLayerPlacement, HostBlockStates, Hooks, KeyRegistration, PayloadId,
    RegisterGuiLayersEvent, Registrar, RegistrationContext, VarInt,
};
// The host's method traits (entities, living, keys…), so their methods resolve.
#[allow(unused_imports)]
use mod_api::prelude::*;
use payloads::*;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

pub(crate) const MOD_ID: &str = "jade";
const LAYER: &str = "jade:overlay";
const TOGGLE: &str = "toggle";

/// How often (client ticks) the client asks the server about its target.
const ASK_EVERY: u32 = 5;

/// Whether the overlay shows (the toggle key flips it).
static SHOWN: AtomicBool = AtomicBool::new(true);
/// Client ticks counted by `client_tick_pre`.
pub(crate) static TICKS: AtomicU32 = AtomicU32::new(0);
/// The client's last mining prediction: block, break fraction, tick it was seen.
pub(crate) static MINING: Mutex<Option<([i32; 3], f32, u32)>> = Mutex::new(None);

pub(crate) fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Default)]
pub struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        let mut toggle = KeyRegistration::new(TOGGLE, "key.keyboard.keypad.0");
        toggle.label = "key.jade.toggle".into();
        r.register_key_mapping(toggle);

        // A refused setting only means its row is missing; its read falls back to the default.
        for spec in settings::specs() {
            let _ = r.register_setting(spec);
        }

        let mut payloads = r.payload_handlers();
        let mut view = payloads.registrar("4").optional();
        let ask_entity = view.play_to_server_typed::<PeekEntity>(
            PayloadId("jade:peek_entity".into()),
            server::answer_entity,
        );
        let answer_entity = view.play_to_client_typed::<EntityInfo>(
            PayloadId("jade:entity_info".into()),
            |value, _| {
                *lock(&ENTITY_INFO) = Some(value);
                Ok(())
            },
        );
        let ask_block = view.play_to_server_typed::<PeekBlock>(
            PayloadId("jade:peek_block".into()),
            server::answer_block,
        );
        let answer_block = view.play_to_client_typed::<BlockInfo>(
            PayloadId("jade:block_info".into()),
            |value, _| {
                *lock(&BLOCK_INFO) = Some(value);
                Ok(())
            },
        );
        let _ = ASK_ENTITY.set(ask_entity);
        let _ = ANSWER_ENTITY.set(answer_entity);
        let _ = ASK_BLOCK.set(ask_block);
        let _ = ANSWER_BLOCK.set(answer_block);
    }

    fn register_gui_layers(&self, _: &RegistrationContext, event: &mut RegisterGuiLayersEvent<'_>) {
        // Under the chat, so an open chat stays readable over it.
        event.layers.push(GuiLayer {
            id: LAYER.into(),
            placement: GuiLayerPlacement::Below("minecraft:chat".into()),
        });
    }

    fn client_tick_pre(&self, host: &mut dyn ClientHost, _: &mut ClientTickPreEvent) {
        while host.consume_click(MOD_ID, TOGGLE) {
            SHOWN.fetch_xor(true, Ordering::Relaxed);
        }
        let tick = TICKS.fetch_add(1, Ordering::Relaxed);
        if !SHOWN.load(Ordering::Relaxed) || tick % ASK_EVERY != 0 {
            return;
        }
        // Errors (server without the mod, not connected) only mean fewer lines.
        match host.crosshair_target() {
            Some(CrosshairTarget::Entity { entity, .. }) => {
                if let Some(ask) = ASK_ENTITY.get() {
                    let _ = ask.send(host, &PeekEntity { id: VarInt(entity.id() as i32) });
                }
            }
            Some(CrosshairTarget::Block { pos, state, .. }) => {
                let has_entity = host.block_states().has_block_entity(state) == Some(true);
                if has_entity && let Some(ask) = ASK_BLOCK.get() {
                    let _ = ask.send(host, &PeekBlock { x: pos.x, y: pos.y, z: pos.z });
                }
            }
            _ => {}
        }
    }

    fn client_mining_predict(&self, _: &dyn ClientHost, event: &mut ClientMiningPredictEvent) -> Flow {
        let p = event.prediction;
        *lock(&MINING) = Some((p.pos, p.progress, TICKS.load(Ordering::Relaxed)));
        Flow::Continue
    }

    fn gui_layer_draw(&self, host: &dyn ClientHost, event: &mut GuiLayerDrawEvent<'_>) {
        if event.layer != LAYER || !SHOWN.load(Ordering::Relaxed) || !host.playing() {
            return;
        }
        if let Some(tooltip) = client::look(host) {
            tooltip.draw(host, event.draw);
        }
    }
}
