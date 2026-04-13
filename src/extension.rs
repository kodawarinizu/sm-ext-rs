use sm_ext_derive::{vtable, vtable_override};

use std::error::Error;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use libc::size_t;

use crate::sharedsys::{IExtension, IExtensionPtr, IShareSys, IShareSysPtr, SMInterface, SMInterfacePtr};

// ------------------------------------------------------------------------------------------------
// IExtensionInterface vtable
// ------------------------------------------------------------------------------------------------

pub type IExtensionInterfacePtr = *mut *mut IExtensionInterfaceVtable;

#[vtable(IExtensionInterfacePtr)]
pub struct IExtensionInterfaceVtable {
    pub GetExtensionVersion: fn() -> i32,
    pub OnExtensionLoad: fn(me: IExtensionPtr, sys: IShareSysPtr, error: *mut c_char, maxlength: size_t, late: bool) -> bool,
    pub OnExtensionUnload: fn() -> (),
    pub OnExtensionsAllLoaded: fn() -> (),
    pub OnExtensionPauseChange: fn(pause: bool) -> (),
    pub QueryInterfaceDrop: fn(interface: SMInterfacePtr) -> bool,
    pub NotifyInterfaceDrop: fn(interface: SMInterfacePtr) -> (),
    pub QueryRunning: fn(error: *mut c_char, maxlength: size_t) -> bool,
    pub IsMetamodExtension: fn() -> bool,
    pub GetExtensionName: fn() -> *const c_char,
    pub GetExtensionURL: fn() -> *const c_char,
    pub GetExtensionTag: fn() -> *const c_char,
    pub GetExtensionAuthor: fn() -> *const c_char,
    pub GetExtensionVerString: fn() -> *const c_char,
    pub GetExtensionDescription: fn() -> *const c_char,
    pub GetExtensionDateString: fn() -> *const c_char,
    pub OnCoreMapStart: fn(edict_list: *mut c_void, edict_count: c_int, client_max: c_int) -> (),
    pub OnDependenciesDropped: fn() -> (),
    pub OnCoreMapEnd: fn() -> (),
}

// ------------------------------------------------------------------------------------------------
// IExtensionInterface / ExtensionMetadata traits
// ------------------------------------------------------------------------------------------------

// There appears to be a bug with the MSVC linker in release mode dropping these symbols when threaded
// compilation is enabled - if you run into undefined symbol errors here try setting code-units to 1.
pub trait IExtensionInterface {
    fn on_extension_load(&mut self, me: IExtension, sys: IShareSys, late: bool) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn on_extension_unload(&mut self) {}
    fn on_extensions_all_loaded(&mut self) {}
    fn on_extension_pause_change(&mut self, pause: bool) {}
    fn on_core_map_start(&mut self, edict_list: *mut c_void, edict_count: i32, client_max: i32) {}
    fn on_core_map_end(&mut self) {}
    fn query_interface_drop(&mut self, interface: SMInterface) -> bool {
        false
    }
    fn notify_interface_drop(&mut self, interface: SMInterface) {}
    fn query_running(&mut self) -> Result<(), CString> {
        Ok(())
    }
    fn on_dependencies_dropped(&mut self) {}
}

pub trait ExtensionMetadata {
    fn get_extension_name(&self) -> &'static std::ffi::CStr;
    fn get_extension_url(&self) -> &'static std::ffi::CStr;
    fn get_extension_tag(&self) -> &'static std::ffi::CStr;
    fn get_extension_author(&self) -> &'static std::ffi::CStr;
    fn get_extension_ver_string(&self) -> &'static std::ffi::CStr;
    fn get_extension_description(&self) -> &'static std::ffi::CStr;
    fn get_extension_date_string(&self) -> &'static std::ffi::CStr;
}

// ------------------------------------------------------------------------------------------------
// IExtensionInterfaceAdapter
// ------------------------------------------------------------------------------------------------

#[repr(C)]
pub struct IExtensionInterfaceAdapter<T: IExtensionInterface + ExtensionMetadata> {
    vtable: *mut IExtensionInterfaceVtable,
    pub delegate: T,
}

impl<T: IExtensionInterface + ExtensionMetadata> Drop for IExtensionInterfaceAdapter<T> {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.vtable));
        }
    }
}

impl<T: IExtensionInterface + ExtensionMetadata> IExtensionInterfaceAdapter<T> {
    pub fn new(delegate: T) -> IExtensionInterfaceAdapter<T> {
        let vtable = IExtensionInterfaceVtable {
            GetExtensionVersion: IExtensionInterfaceAdapter::<T>::get_extension_version,
            OnExtensionLoad: IExtensionInterfaceAdapter::<T>::on_extension_load,
            OnExtensionUnload: IExtensionInterfaceAdapter::<T>::on_extension_unload,
            OnExtensionsAllLoaded: IExtensionInterfaceAdapter::<T>::on_extensions_all_loaded,
            OnExtensionPauseChange: IExtensionInterfaceAdapter::<T>::on_extension_pause_change,
            QueryInterfaceDrop: IExtensionInterfaceAdapter::<T>::query_interface_drop,
            NotifyInterfaceDrop: IExtensionInterfaceAdapter::<T>::notify_interface_drop,
            QueryRunning: IExtensionInterfaceAdapter::<T>::query_running,
            IsMetamodExtension: IExtensionInterfaceAdapter::<T>::is_metamod_extension,
            GetExtensionName: IExtensionInterfaceAdapter::<T>::get_extension_name,
            GetExtensionURL: IExtensionInterfaceAdapter::<T>::get_extension_url,
            GetExtensionTag: IExtensionInterfaceAdapter::<T>::get_extension_tag,
            GetExtensionAuthor: IExtensionInterfaceAdapter::<T>::get_extension_author,
            GetExtensionVerString: IExtensionInterfaceAdapter::<T>::get_extension_ver_string,
            GetExtensionDescription: IExtensionInterfaceAdapter::<T>::get_extension_description,
            GetExtensionDateString: IExtensionInterfaceAdapter::<T>::get_extension_date_string,
            OnCoreMapStart: IExtensionInterfaceAdapter::<T>::on_core_map_start,
            OnDependenciesDropped: IExtensionInterfaceAdapter::<T>::on_dependencies_dropped,
            OnCoreMapEnd: IExtensionInterfaceAdapter::<T>::on_core_map_end,
        };

        IExtensionInterfaceAdapter { vtable: Box::into_raw(Box::new(vtable)), delegate }
    }

    #[vtable_override]
    unsafe fn get_extension_version(this: IExtensionInterfacePtr) -> i32 {
        8
    }

    #[vtable_override]
    unsafe fn on_extension_load(this: IExtensionInterfacePtr, me: IExtensionPtr, sys: IShareSysPtr, error: *mut c_char, maxlength: size_t, late: bool) -> bool {
        let result = std::panic::catch_unwind(|| (*this.cast::<Self>()).delegate.on_extension_load(IExtension(me), IShareSys(sys), late));

        match result {
            Ok(result) => match result {
                Ok(_) => true,
                Err(err) => {
                    let err = CString::new(err.to_string()).unwrap_or_else(|_| c_str_macro::c_str!("load error message contained NUL byte").into());
                    libc::strncpy(error, err.as_ptr(), maxlength);
                    false
                }
            },
            Err(err) => {
                let msg = format!(
                    "load panicked: {}",
                    if let Some(str_slice) = err.downcast_ref::<&'static str>() {
                        str_slice
                    } else if let Some(string) = err.downcast_ref::<String>() {
                        string
                    } else {
                        "unknown message"
                    }
                );

                let msg = CString::new(msg).unwrap_or_else(|_| c_str_macro::c_str!("load panic message contained NUL byte").into());
                libc::strncpy(error, msg.as_ptr(), maxlength);
                false
            }
        }
    }

    #[vtable_override]
    unsafe fn on_extension_unload(this: IExtensionInterfacePtr) {
        let _ = std::panic::catch_unwind(|| (*this.cast::<Self>()).delegate.on_extension_unload());
    }

    #[vtable_override]
    unsafe fn on_extensions_all_loaded(this: IExtensionInterfacePtr) {
        let _ = std::panic::catch_unwind(|| (*this.cast::<Self>()).delegate.on_extensions_all_loaded());
    }

    #[vtable_override]
    unsafe fn on_extension_pause_change(this: IExtensionInterfacePtr, pause: bool) {
        let _ = std::panic::catch_unwind(|| (*this.cast::<Self>()).delegate.on_extension_pause_change(pause));
    }

    #[vtable_override]
    unsafe fn query_interface_drop(this: IExtensionInterfacePtr, interface: SMInterfacePtr) -> bool {
        let result = std::panic::catch_unwind(|| (*this.cast::<Self>()).delegate.query_interface_drop(SMInterface(interface)));

        match result {
            Ok(result) => result,
            Err(_) => false,
        }
    }

    #[vtable_override]
    unsafe fn notify_interface_drop(this: IExtensionInterfacePtr, interface: SMInterfacePtr) {
        let _ = std::panic::catch_unwind(|| (*this.cast::<Self>()).delegate.notify_interface_drop(SMInterface(interface)));
    }

    #[vtable_override]
    unsafe fn query_running(this: IExtensionInterfacePtr, error: *mut c_char, maxlength: size_t) -> bool {
        let result = std::panic::catch_unwind(|| match (*this.cast::<Self>()).delegate.query_running() {
            Ok(_) => true,
            Err(str) => {
                libc::strncpy(error, str.as_ptr(), maxlength);
                false
            }
        });

        match result {
            Ok(result) => result,
            Err(_) => {
                libc::strncpy(error, c_str_macro::c_str!("query running callback panicked").as_ptr(), maxlength);
                false
            }
        }
    }

    #[vtable_override]
    unsafe fn is_metamod_extension(this: IExtensionInterfacePtr) -> bool {
        false
    }

    #[vtable_override]
    unsafe fn get_extension_name(this: IExtensionInterfacePtr) -> *const c_char {
        (*this.cast::<Self>()).delegate.get_extension_name().as_ptr()
    }

    #[vtable_override]
    unsafe fn get_extension_url(this: IExtensionInterfacePtr) -> *const c_char {
        (*this.cast::<Self>()).delegate.get_extension_url().as_ptr()
    }

    #[vtable_override]
    unsafe fn get_extension_tag(this: IExtensionInterfacePtr) -> *const c_char {
        (*this.cast::<Self>()).delegate.get_extension_tag().as_ptr()
    }

    #[vtable_override]
    unsafe fn get_extension_author(this: IExtensionInterfacePtr) -> *const c_char {
        (*this.cast::<Self>()).delegate.get_extension_author().as_ptr()
    }

    #[vtable_override]
    unsafe fn get_extension_ver_string(this: IExtensionInterfacePtr) -> *const c_char {
        (*this.cast::<Self>()).delegate.get_extension_ver_string().as_ptr()
    }

    #[vtable_override]
    unsafe fn get_extension_description(this: IExtensionInterfacePtr) -> *const c_char {
        (*this.cast::<Self>()).delegate.get_extension_description().as_ptr()
    }

    #[vtable_override]
    unsafe fn get_extension_date_string(this: IExtensionInterfacePtr) -> *const c_char {
        (*this.cast::<Self>()).delegate.get_extension_date_string().as_ptr()
    }

    #[vtable_override]
    unsafe fn on_core_map_start(this: IExtensionInterfacePtr, edict_list: *mut c_void, edict_count: c_int, client_max: c_int) {
        let _ = std::panic::catch_unwind(|| (*this.cast::<Self>()).delegate.on_core_map_start(edict_list, edict_count, client_max));
    }

    #[vtable_override]
    unsafe fn on_dependencies_dropped(this: IExtensionInterfacePtr) {
        let _ = std::panic::catch_unwind(|| (*this.cast::<Self>()).delegate.on_dependencies_dropped());
    }

    #[vtable_override]
    unsafe fn on_core_map_end(this: IExtensionInterfacePtr) {
        let _ = std::panic::catch_unwind(|| (*this.cast::<Self>()).delegate.on_core_map_end());
    }
}
