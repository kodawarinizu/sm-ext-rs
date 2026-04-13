use sm_ext_derive::{vtable, SMInterfaceApi};

use std::error::Error;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_uint, c_void};
use std::str::Utf8Error;
use libc::size_t;

use crate::types::{FeatureStatus, FeatureType, IdentityTokenPtr, NativeInfo};
use crate::virtual_call;

// ------------------------------------------------------------------------------------------------
// IExtension
// ------------------------------------------------------------------------------------------------

pub type IExtensionPtr = *mut *mut IExtensionVtable;

#[vtable(IExtensionPtr)]
pub struct IExtensionVtable {
    pub IsLoaded: fn() -> bool,
    pub GetAPI: fn() -> *mut c_void,
    pub GetFilename: fn() -> *const c_char,
    pub GetIdentity: fn() -> IdentityTokenPtr,
    _FindFirstDependency: fn() -> *mut c_void,
    _FindNextDependency: fn() -> *mut c_void,
    _FreeDependencyIterator: fn() -> *mut c_void,
    pub IsRunning: fn(error: *mut c_char, maxlength: size_t) -> bool,
    pub IsExternal: fn() -> bool,
}

#[derive(Debug)]
pub enum IsRunningError<'str> {
    WithReason(&'str str),
    InvalidReason(Utf8Error),
}

impl std::fmt::Display for IsRunningError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        std::fmt::Debug::fmt(self, f)
    }
}

impl Error for IsRunningError<'_> {}

#[derive(Debug)]
pub struct IExtension(pub(crate) IExtensionPtr);

impl IExtension {
    pub fn is_loaded(&self) -> bool {
        unsafe { virtual_call!(IsLoaded, self.0) }
    }

    pub fn get_api(&self) -> *mut c_void {
        unsafe { virtual_call!(GetAPI, self.0) }
    }

    pub fn get_filename(&self) -> Result<&str, Utf8Error> {
        unsafe {
            let c_name = virtual_call!(GetFilename, self.0);
            CStr::from_ptr(c_name).to_str()
        }
    }

    pub fn get_identity(&self) -> IdentityTokenPtr {
        unsafe { virtual_call!(GetIdentity, self.0) }
    }

    pub fn is_running(&self) -> Result<(), IsRunningError<'_>> {
        unsafe {
            let mut c_error = [0 as c_char; 256];
            let result = virtual_call!(IsRunning, self.0, c_error.as_mut_ptr(), c_error.len());

            if result {
                Ok(())
            } else {
                match CStr::from_ptr(c_error.as_ptr()).to_str() {
                    Ok(error) => Err(IsRunningError::WithReason(error)),
                    Err(e) => Err(IsRunningError::InvalidReason(e)),
                }
            }
        }
    }

    pub fn is_external(&self) -> bool {
        unsafe { virtual_call!(IsExternal, self.0) }
    }
}

// ------------------------------------------------------------------------------------------------
// SMInterface / RequestableInterface / SMInterfaceApi
// ------------------------------------------------------------------------------------------------

pub type SMInterfacePtr = *mut *mut SMInterfaceVtable;

#[vtable(SMInterfacePtr)]
pub struct SMInterfaceVtable {
    pub GetInterfaceVersion: fn() -> c_uint,
    pub GetInterfaceName: fn() -> *const c_char,
    pub IsVersionCompatible: fn(version: c_uint) -> bool,
}

pub trait RequestableInterface {
    fn get_interface_name() -> &'static str;
    fn get_interface_version() -> u32;

    /// # Safety
    ///
    /// Only for use internally by [`IShareSys::request_interface`], which always knows the correct type.
    unsafe fn from_raw_interface(iface: SMInterface) -> Self;
}

pub trait SMInterfaceApi {
    fn get_interface_version(&self) -> u32;
    fn get_interface_name(&self) -> &str;
    fn is_version_compatible(&self, version: u32) -> bool;
}

#[derive(Debug, SMInterfaceApi)]
pub struct SMInterface(pub(crate) SMInterfacePtr);

// ------------------------------------------------------------------------------------------------
// Stub vtables (IFeatureProvider / IPluginRuntime)
// ------------------------------------------------------------------------------------------------

pub type IFeatureProviderPtr = *mut *mut IFeatureProviderVtable;

#[vtable(IFeatureProviderPtr)]
pub struct IFeatureProviderVtable {}

pub type IPluginRuntimePtr = *mut *mut IPluginRuntimeVtable;

#[vtable(IPluginRuntimePtr)]
pub struct IPluginRuntimeVtable {}

// ------------------------------------------------------------------------------------------------
// IShareSys
// ------------------------------------------------------------------------------------------------

pub type IShareSysPtr = *mut *mut IShareSysVtable;

#[vtable(IShareSysPtr)]
pub struct IShareSysVtable {
    pub AddInterface: fn(myself: IExtensionPtr, iface: SMInterfacePtr) -> bool,
    pub RequestInterface: fn(iface_name: *const c_char, iface_vers: c_uint, myself: IExtensionPtr, iface: *mut SMInterfacePtr) -> bool,
    pub AddNatives: fn(myself: IExtensionPtr, natives: *const NativeInfo) -> (),
    pub CreateIdentType: fn(name: *const c_char) -> crate::IdentityType,
    pub FindIdentType: fn(name: *const c_char) -> crate::IdentityType,
    pub CreateIdentity: fn(ident_type: crate::IdentityType, ptr: *mut c_void) -> IdentityTokenPtr,
    pub DestroyIdentType: fn(ident_type: crate::IdentityType) -> (),
    pub DestroyIdentity: fn(identity: IdentityTokenPtr) -> (),
    pub AddDependency: fn(myself: IExtensionPtr, filename: *const c_char, require: bool, autoload: bool) -> (),
    pub RegisterLibrary: fn(myself: IExtensionPtr, name: *const c_char) -> (),
    _OverrideNatives: fn(myself: IExtensionPtr, natives: *const NativeInfo) -> (),
    pub AddCapabilityProvider: fn(myself: IExtensionPtr, provider: IFeatureProviderPtr, name: *const c_char) -> (),
    pub DropCapabilityProvider: fn(myself: IExtensionPtr, provider: IFeatureProviderPtr, name: *const c_char) -> (),
    pub TestFeature: fn(rt: IPluginRuntimePtr, feature_type: FeatureType, name: *const c_char) -> FeatureStatus,
}

#[derive(Debug)]
pub enum RequestInterfaceError {
    InvalidName(std::ffi::NulError),
    InvalidInterface(String, u32),
}

impl std::fmt::Display for RequestInterfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            RequestInterfaceError::InvalidName(err) => write!(f, "invalid interface name: {}", err),
            RequestInterfaceError::InvalidInterface(name, ver) => write!(f, "failed to get {} interface version {}", name, ver),
        }
    }
}

impl Error for RequestInterfaceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RequestInterfaceError::InvalidName(err) => Some(err),
            RequestInterfaceError::InvalidInterface(_, _) => None,
        }
    }
}

#[derive(Debug)]
pub struct IShareSys(pub(crate) IShareSysPtr);

impl IShareSys {
    pub fn request_interface<I: RequestableInterface>(&self, myself: &IExtension) -> Result<I, RequestInterfaceError> {
        let iface = self.request_raw_interface(myself, I::get_interface_name(), I::get_interface_version())?;
        unsafe { Ok(I::from_raw_interface(iface)) }
    }

    pub fn request_raw_interface(&self, myself: &IExtension, name: &str, version: u32) -> Result<SMInterface, RequestInterfaceError> {
        let c_name = CString::new(name).map_err(RequestInterfaceError::InvalidName)?;

        unsafe {
            let mut iface: SMInterfacePtr = std::ptr::null_mut();
            let res = virtual_call!(RequestInterface, self.0, c_name.as_ptr(), version, myself.0, &mut iface);

            if res {
                Ok(SMInterface(iface))
            } else {
                Err(RequestInterfaceError::InvalidInterface(name.into(), version))
            }
        }
    }

    /// # Safety
    ///
    /// This should be be used via the [`register_natives!`] macro only.
    pub unsafe fn add_natives(&self, myself: &IExtension, natives: *const NativeInfo) {
        virtual_call!(AddNatives, self.0, myself.0, natives)
    }
}
