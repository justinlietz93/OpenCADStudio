//! "Make Open CAD Studio the default for .dwg / .dxf" — the platform-specific
//! plumbing behind the one-time first-launch prompt (see `app::update`'s
//! `AssocPrompt*` handlers).
//!
//! Each OS guards default file associations differently, so there is no single
//! cross-platform call:
//!
//!   * Windows — the actual default is protected by a per-user UserChoice hash
//!     an app cannot forge. The supported path is to open the OS's own
//!     per-app default-programs dialog via
//!     `IApplicationAssociationRegistrationUI::LaunchAdvancedAssociationUI`,
//!     passing the RegisteredApplications name the MSI registered
//!     ("Open CAD Studio"). The user confirms there.
//!   * Linux — `xdg-mime default` writes the association into the user's
//!     `mimeapps.list`; no separate consent step. The .desktop file already
//!     declares the matching `MimeType=` entries.
//!   * macOS — LaunchServices' `LSSetDefaultRoleHandlerForContentType` binds
//!     the DWG/DXF UTIs (declared in the bundle's Info.plist) to this app's
//!     bundle id.
//!
//! The work runs on a dedicated thread because the Windows dialog is modal and
//! would otherwise block the iced executor; the result is delivered back
//! through a oneshot channel that the async wrapper awaits.

/// Reverse-DNS bundle / app id, shared by the macOS handler binding and the
/// Linux desktop-file name. Matches `CFBundleIdentifier` in packaging/Info.plist
/// and the installed `*.desktop` basename.
#[cfg(any(target_os = "macos", target_os = "linux"))]
const APP_ID: &str = "io.github.HakanSeven12.OpenCadStudio";

/// Try to make this app the default handler for .dwg and .dxf. Returns a short
/// human-readable status string on success, or an error message on failure.
pub async fn set_default_app() -> Result<String, String> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::Builder::new()
        .name("set-default-app".into())
        .spawn(move || {
            let _ = tx.send(set_default_app_blocking());
        })
        .map_err(|e| format!("could not start the default-app helper: {e}"))?;
    rx.await
        .unwrap_or_else(|_| Err("the default-app helper was cancelled".to_string()))
}

fn set_default_app_blocking() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::set_default()
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::set_default()
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::set_default()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Err("Setting the default application isn't supported on this platform.".to_string())
    }
}

// ── Windows ────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod windows_impl {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::core::{GUID, HRESULT};
    use windows_sys::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };

    // CLSID_ApplicationAssociationRegistrationUI
    // {1968106D-F3B5-44CF-890E-116FCB9ECEF1}
    const CLSID_APP_ASSOC_UI: GUID = GUID {
        data1: 0x1968106D,
        data2: 0xF3B5,
        data3: 0x44CF,
        data4: [0x89, 0x0E, 0x11, 0x6F, 0xCB, 0x9E, 0xCE, 0xF1],
    };
    // IID_IApplicationAssociationRegistrationUI
    // {1F76A169-F994-40AC-8FC8-0959E8874710}
    const IID_APP_ASSOC_UI: GUID = GUID {
        data1: 0x1F76A169,
        data2: 0xF994,
        data3: 0x40AC,
        data4: [0x8F, 0xC8, 0x09, 0x59, 0xE8, 0x87, 0x47, 0x10],
    };

    // Hand-rolled vtable for IApplicationAssociationRegistrationUI (IUnknown +
    // its single method). windows-sys ships raw COM, so we drive the interface
    // through the vtable directly rather than depend on generated wrappers.
    #[repr(C)]
    struct IAppAssocUiVtbl {
        query_interface:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
        add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        launch_advanced_association_ui:
            unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    }
    #[repr(C)]
    struct IAppAssocUi {
        vtbl: *const IAppAssocUiVtbl,
    }

    // Must match the RegisteredApplications value name in packaging/windows/main.wxs.
    const APP_REGISTRY_NAME: &str = "Open CAD Studio";

    pub(super) fn set_default() -> Result<String, String> {
        unsafe {
            let co = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED);
            // S_OK (0) / S_FALSE (1) mean we initialised COM and must balance it
            // with CoUninitialize. Any other value: another init already owns the
            // thread's apartment, so we leave it alone.
            let owns_com = co == 0 || co == 1;

            let result = (|| {
                let mut obj: *mut c_void = std::ptr::null_mut();
                let hr = CoCreateInstance(
                    &CLSID_APP_ASSOC_UI,
                    std::ptr::null_mut(),
                    CLSCTX_INPROC_SERVER,
                    &IID_APP_ASSOC_UI,
                    &mut obj,
                );
                if hr < 0 || obj.is_null() {
                    return Err(format!(
                        "could not open the Windows default-apps dialog (0x{:08X})",
                        hr as u32
                    ));
                }
                let this = obj as *mut IAppAssocUi;
                let name: Vec<u16> = std::ffi::OsStr::new(APP_REGISTRY_NAME)
                    .encode_wide()
                    .chain(Some(0))
                    .collect();
                let hr = ((*(*this).vtbl).launch_advanced_association_ui)(obj, name.as_ptr());
                ((*(*this).vtbl).release)(obj);
                if hr < 0 {
                    return Err(format!(
                        "the Windows default-apps dialog returned an error (0x{:08X})",
                        hr as u32
                    ));
                }
                Ok("Opened the Windows default-apps dialog — tick .dwg / .dxf there.".to_string())
            })();

            if owns_com {
                CoUninitialize();
            }
            result
        }
    }
}

// ── Linux ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::APP_ID;

    pub(super) fn set_default() -> Result<String, String> {
        let desktop = format!("{APP_ID}.desktop");
        let status = std::process::Command::new("xdg-mime")
            .args(["default", &desktop, "image/vnd.dwg", "image/vnd.dxf"])
            .status()
            .map_err(|e| format!("could not run xdg-mime: {e}"))?;
        if status.success() {
            Ok("Open CAD Studio is now the default for .dwg and .dxf files.".to_string())
        } else {
            Err(format!("xdg-mime exited with {status}"))
        }
    }
}

// ── macOS ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::APP_ID;
    use std::ffi::c_void;

    #[repr(C)]
    struct __CFString {
        _private: [u8; 0],
    }
    type CFStringRef = *const __CFString;
    type CFAllocatorRef = *const c_void;
    type OSStatus = i32;
    type LSRolesMask = u32;

    const KCFSTRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const KLS_ROLES_ALL: LSRolesMask = 0xFFFF_FFFF;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFAllocatorDefault: CFAllocatorRef;
        fn CFStringCreateWithBytes(
            alloc: CFAllocatorRef,
            bytes: *const u8,
            num_bytes: isize,
            encoding: u32,
            is_external_representation: u8,
        ) -> CFStringRef;
        fn CFRelease(cf: *const c_void);
    }

    // LaunchServices lives under the CoreServices umbrella framework.
    #[link(name = "CoreServices", kind = "framework")]
    extern "C" {
        fn LSSetDefaultRoleHandlerForContentType(
            in_content_type: CFStringRef,
            in_role: LSRolesMask,
            in_handler_bundle_id: CFStringRef,
        ) -> OSStatus;
    }

    fn cfstr(s: &str) -> CFStringRef {
        unsafe {
            CFStringCreateWithBytes(
                kCFAllocatorDefault,
                s.as_ptr(),
                s.len() as isize,
                KCFSTRING_ENCODING_UTF8,
                0,
            )
        }
    }

    pub(super) fn set_default() -> Result<String, String> {
        let bundle = cfstr(APP_ID);
        if bundle.is_null() {
            return Err("could not build the bundle-id string".to_string());
        }
        // UTIs declared in packaging/Info.plist's CFBundleDocumentTypes.
        let mut last_err: Option<String> = None;
        for uti in ["com.autodesk.dwg", "com.autodesk.dxf"] {
            let ct = cfstr(uti);
            if ct.is_null() {
                last_err = Some("could not build the content-type string".to_string());
                continue;
            }
            let st = unsafe { LSSetDefaultRoleHandlerForContentType(ct, KLS_ROLES_ALL, bundle) };
            unsafe { CFRelease(ct as *const c_void) };
            if st != 0 {
                last_err = Some(format!("LaunchServices error {st} for {uti}"));
            }
        }
        unsafe { CFRelease(bundle as *const c_void) };
        match last_err {
            None => Ok("Open CAD Studio is now the default for .dwg and .dxf files.".to_string()),
            Some(e) => Err(e),
        }
    }
}
