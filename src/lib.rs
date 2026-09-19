use mod_api_stable::*;

mod native_effects;
mod driver;
mod extension;
mod buff_visuals;
mod helpers;
mod logger;
mod state;
mod paragon;
mod paragon_passive;

fn init(_host: &StableHost) -> StableMod {
    let mut reg = StableMod::new("my_mod");
    native_effects::register(&mut reg);
    reg.add_champion(paragon::Paragon);
    reg.set_match_hook(driver::DriverHook::new());
    reg.set_extension(extension::VfxExtension);
    reg
}

declare_stable_mod!(init);