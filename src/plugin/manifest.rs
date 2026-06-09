// Plugin identity and capability declaration.

/// Host plugin API version. Bump when HostApi breaks compatibility.
pub const API_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApiVersion {
    pub major: u32,
}

impl ApiVersion {
    pub const CURRENT: Self = Self { major: API_VERSION };

    pub fn is_compatible_with(host: ApiVersion) -> bool {
        Self::CURRENT.major == host.major
    }
}

/// Static metadata every plugin supplies at registration time.
/// Keep fields in sync with `plugin.toml` beside the package.
#[derive(Clone, Copy, Debug)]
pub struct PluginManifest {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    pub api_version: ApiVersion,
    /// Sort key for add-on ribbon tabs (lower = further left among plugins).
    pub ribbon_order: i32,
    pub xdata_apps: &'static [&'static str],
    pub command_prefixes: &'static [&'static str],
}