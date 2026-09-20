# Anime Characters for TFM2

Adds four anime-inspired characters to **Teamfight Manager 2**: **Saitama**, **Paragon**, **Naruto** and **Minato**.

Each champion ships with its own sprite sheet, animation data, VFX, ability icons and sound effects. Three of them (Saitama, Paragon and Naruto) drive their abilities through custom native code, so this mod includes compiled Rust alongside the data files.

---

# Saitama

**Category:** Melee · **Tags:** AD, Melee

| HP | Attack | Defence | Magic Resist | Move Speed | Crit |
|----|--------|---------|--------------|------------|------|
| 1000 | 100 | 25 | 25 | 1100 | 0 |

**Berserker** (Passive)
Saitama's passive accelerates his cooldowns as he stays in the fight, stacking cooldown reduction up to a hard cap.

**Skill 1** — Melee range, short cooldown.
A close-range flurry. Driven by custom native code with a dedicated sound effect.

**Skill 2** — Melee range, short cooldown.
A second, longer flurry — the extended version of his Skill 1 combo, also natively scripted.

**One-Punch** (Ultimate) — Long range.
Saitama's only ranged threat. Winds up with a full-screen caster effect before landing the finisher.

---

# Paragon

**Category:** Melee · **Tags:** AD, Melee, Tank, CC

| HP | Attack | Defence | Magic Resist | Move Speed | Crit |
|----|--------|---------|--------------|------------|------|
| 1300 | 90 | 35 | 30 | 1000 | 0 |

The tankiest of the four, built to soak damage and lock targets down.

**Skill 1** — Short cooldown.
Fully custom native ability.

**Skill 2** — Short cooldown.
Fully custom native ability.

**Ultimate** — Very long cooldown.
Fully custom native ability — Paragon's entire kit runs through native code rather than stock effects.

*Paragon also carries four VFX: a damage-over-time pulse, an aura, a grab impact and a shatter chain.*

---

# Naruto

**Category:** Melee · **Tags:** AD, Melee, CC

| HP | Attack | Defence | Magic Resist | Move Speed | Crit | HP Regen |
|----|--------|---------|--------------|------------|------|----------|
| 1120 | 82 | 26 | 22 | 1050 | 12 | 3 |

**Skill 1** — Medium range.
A natively scripted attack with a clone-spawn visual.

**Skill 2** — Medium range, with crowd control.
Dashes to the target and **stuns** it. Natively scripted, with a dedicated sound effect.

**Ultimate** — Very long cooldown, long range.
An **area-of-effect** finisher (circle around the caster) that applies **Airborne**. Natively scripted, with a Rasengan sound effect.

*Naruto ships with two VFX sets — a DSi-style set and a GBA-style set — so you can pick the look you prefer.*

---

# Minato

**Category:** Assassin · **Tags:** AD, Range

| HP | Attack | Defence | Magic Resist | Move Speed | Crit |
|----|--------|---------|--------------|------------|------|
| 1050 | 75 | 20 | 20 | 1250 | 16 |

The fastest of the four and the only true ranged Assassin.

**Skill 1** — Long range.
Rushes **behind** the target and strikes. Uses a buff-state switch, so its behaviour changes depending on Minato's current buffs.

**Skill 2** — Long range, with crowd control.
Dashes to the target and **stuns** it.

**Ultimate** — Very long cooldown.
A **delayed area-of-effect** strike (circle around the caster) that also applies buffs to Minato. Natively-flavoured sound included.

> **Note on the ID:** Minato's champion ID in the data files is `my_1st_mod_champion`, because he was the first champion built for this mod folder. His assets are named accordingly (`my_1st_mod_champion#sheet.png`, `my_1st_mod_champion_skill.png`, …). Renaming him to `minato` would require renaming every asset plus the registry entries — it is cosmetic only and deliberately left alone.

---

# Installation

1. Subscribe/enable the mod in the **Teamfight Manager 2** in-game mod manager, **or**
2. Copy the contents of this repository into:
   ```
   C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2\mods\my_mod\
   ```
   The folder name must match the crate name for the DLL to load.

# Building from source

The Rust side is a **Stable-ABI** mod — it builds with any Rust toolchain and keeps loading across game updates (no pinned nightly, no prebuilt engine rlibs).

```bash
cargo build --release
```

The resulting `target/release/my_mod.dll` gets copied next to the data files. The SDK crate (`mod-api-stable`) is plain source; the uploader installs it at `../mod-api-stable` if it is missing.

# Important

This mod currently supports the **English locale only**. You can use it with other languages, but the characters' names and skill descriptions will fall back to raw translation keys.

# Known Issues

- Saitama's Skill 1 and Skill 2 frames are larger than his idle/run frames, so he appears slightly bigger during those two abilities.
- Balance numbers are still being tuned across all four champions. Feedback is very welcome.

# Credits

Thanks to the Teamfight Manager 2 modding community for the SDK setup, documentation and general help.

# Legalese

This is a free, fan-made mod. I am not affiliated with the creators, publishers or licensors of *One-Punch Man* or *Naruto*, nor with Teamfight Manager 2's developers. Character concepts, names, artwork and audio are the property of their respective owners. All sprites, icons and sound effects are used here for non-commercial, transformative fan purposes.

If you are a rights holder and would like anything removed, please open an issue and it will be taken down promptly.

**[Code Mod Notice]**

This Workshop item contains native/executable code files. Enabling it allows code to run inside the game process. Use only mods from creators you trust.

Files: `my_mod.dll`

Runs on: Windows
