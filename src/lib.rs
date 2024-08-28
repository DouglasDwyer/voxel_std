include!(concat!(env!("OUT_DIR"), "/standard_plugins.rs"));

/// References a precompiled, builtin mod.
pub struct StandardMod {
    /// The name of the mod.
    pub name: &'static str,
    /// The binary WASM data for the mod.
    pub module: &'static [u8]
}