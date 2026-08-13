#![cfg(target_os = "android")]

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

mod error;
mod mobile;

pub use error::{Error, Result};
pub use mobile::{BuildRequest, NativeResponse, ToolchainRequest};
use mobile::FunoAndroid;

pub trait FunoAndroidExt<R: Runtime> {
    fn funo_android(&self) -> &FunoAndroid<R>;
}

impl<R: Runtime, T: Manager<R>> FunoAndroidExt<R> for T {
    fn funo_android(&self) -> &FunoAndroid<R> {
        self.state::<FunoAndroid<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("funo-android")
        .setup(|app, api| {
            let plugin = mobile::init(app, api)?;
            app.manage(plugin);
            Ok(())
        })
        .build()
}
