# Jade

What you are looking at, for [Horizon](https://playhorizon.dev) (Minecraft: Java Edition 26.3 in Rust, mod API 0.2): a tooltip at the top of the screen for the block or entity under the crosshair. A WebAssembly mod, in English, French and Arabic; in a right-to-left language the box mirrors (icon on the right, lines and item rows from the right).

## Screenshots

| Block | Crop growth | Furnace |
| --- | --- | --- |
| ![A grass block with its name and mod](docs/screenshots/block.png) | ![Wheat with its growth percentage](docs/screenshots/crop-growth.png) | ![A furnace with its cooking progress](docs/screenshots/furnace.png) |

## What it shows

- **Block**: icon, name (in the game's language), originating mod, and depending on the block:
  - crop growth (wheat, carrots, potatoes, beetroots, melon and pumpkin stems, cocoa, nether wart, sweet berries, torchflower, pitcher plant, and any modded crop with an `age` property): "Growth: 57%" or "Mature";
  - honey level and bee count of a hive, compost level, respawn anchor charges, farmland moisture, turtle and sniffer egg hatching, cake slices left;
  - redstone: power (wire, daylight detector, target, sculk sensors, weighted pressure plates), repeater delay, comparator mode, lever state;
  - a note block's note and instrument, a jukebox's disc, a spawner's mob;
  - a container's contents (chest, barrel, hopper, furnace, campfire…), merged by item, up to 18 icons with counts; "Unopened loot" for a structure chest never opened; nothing for a locked container;
  - cooking progress of a furnace, smoker or blast furnace, brewing progress of a brewing stand;
  - whether the item in your hand can harvest the block;
  - a progress bar while mining.
- **Entity**: name, health (❤ 18 / 20), originating mod, and depending on the entity:
  - animals: time until adulthood, until they can breed again, love mode, next egg (chickens);
  - tamed animals: owner;
  - horses, donkeys, mules, llamas: speed (blocks/s), jump height (blocks), llama strength;
  - villagers: level (and profession when the name doesn't already say it); a zombie villager being cured: time left;
  - active effects (name, level, duration);
  - item frame: the item it holds; primed TNT: time until it explodes.

**Numpad 0** shows or hides the tooltip (rebindable in Options > Controls).

## Settings

**Options > Mods > Jade > Config**: theme (Vanilla, Dark, Light, Waila), position (top centre, left, right), size (0.5 to 2), and one toggle per kind of information.

Health, animal, villager and horse data, effects, contents and furnace/brewing progress are known only to the server: the mod asks for them a few times per second, for a target within 24 blocks. On a server without this mod the tooltip still works, without those lines.

## Install

1. Copy `jade.hmod` into `%APPDATA%\horizon\horizon-mods\`.
2. Add the line `mod.jade:true` to `%APPDATA%\horizon\horizon-mods.txt`.
3. Restart Horizon.

Built for Horizon Nightly `f037744` (2026-11-21). For multiplayer, install it on the client; on the server too for the server-only lines.

## Build

```powershell
cargo horizon-mod build --wasm
```

The result is `target\horizon-mod\jade.hmod`.

## Licence and credits

CC BY-NC-SA 4.0 (see [LICENSE](LICENSE) and [NOTICE](NOTICE)): non-commercial use only. Adapted from [Jade](https://github.com/Snownee/Jade) by Snownee and contributors and from [jade-horizon](https://github.com/Pierddev/jade-horizon) by Pierddev. No Minecraft assets or data are included. Not affiliated with Snownee, Pierddev or the Horizon project.
