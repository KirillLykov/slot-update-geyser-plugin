use {crate::plugin::Plugin, agave_geyser_plugin_interface::geyser_plugin_interface::GeyserPlugin};

pub mod broadcaster;
pub mod config;
pub mod message;
pub mod plugin;

#[no_mangle]
#[allow(improper_ctypes_definitions)]
pub unsafe extern "C" fn _create_plugin() -> *mut dyn GeyserPlugin {
    let plugin = Plugin::default();
    let plugin: Box<dyn GeyserPlugin> = Box::new(plugin);
    Box::into_raw(plugin)
}
