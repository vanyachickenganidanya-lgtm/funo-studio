use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainRequest {
    pub project_root: String,
    pub minecraft_version: String,
    pub loader: String,
    pub check_updates: bool,
    pub destination_root: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildRequest {
    pub project_root: String,
    pub source: String,
    pub minecraft_version: String,
    pub loader: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NativeResponse {
    #[serde(default)]
    pub value: String,
}

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<FunoAndroid<R>> {
    let handle = api.register_android_plugin("dev.funo.studio.android", "FunoAndroidPlugin")?;
    Ok(FunoAndroid(handle))
}

pub struct FunoAndroid<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> FunoAndroid<R> {
    pub fn toolchain_status<T: DeserializeOwned>(&self, request: ToolchainRequest) -> crate::Result<T> {
        self.0
            .run_mobile_plugin("toolchainStatus", request)
            .map_err(Into::into)
    }

    pub fn install_toolchain<T: DeserializeOwned>(&self, request: ToolchainRequest) -> crate::Result<T> {
        self.0
            .run_mobile_plugin("installToolchain", request)
            .map_err(Into::into)
    }

    pub fn build_minecraft<T: DeserializeOwned>(&self, request: BuildRequest) -> crate::Result<T> {
        self.0
            .run_mobile_plugin("buildMinecraft", request)
            .map_err(Into::into)
    }

    pub fn open_launcher(&self) -> crate::Result<NativeResponse> {
        self.0
            .run_mobile_plugin("openLauncher", ())
            .map_err(Into::into)
    }
}
