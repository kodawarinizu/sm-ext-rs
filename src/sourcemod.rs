use sm_ext_derive::{vtable, SMInterfaceApi};

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use libc::size_t;

use crate::plugin::IPluginContextPtr;
use crate::sharedsys::{IExtension, IExtensionPtr, RequestableInterface, SMInterface, SMInterfaceApi};
use crate::types::cell_t;
use crate::{virtual_call, virtual_call_varargs};

// ------------------------------------------------------------------------------------------------
// PathType
// ------------------------------------------------------------------------------------------------

/// Describes various ways of formatting a base path.
#[repr(C)]
#[derive(Debug)]
pub enum PathType {
    /// No base path
    Path_None = 0,
    /// Base path is absolute mod folder
    Path_Game = 1,
    /// Base path is absolute to SourceMod
    Path_SM = 2,
    /// Base path is relative to SourceMod
    Path_SM_Rel = 3,
}

// ------------------------------------------------------------------------------------------------
// ISourceMod
// ------------------------------------------------------------------------------------------------

pub type GameFrameHookFunc = unsafe extern "C" fn(simulating: bool);

pub type ISourceModPtr = *mut *mut ISourceModVtable;

#[vtable(ISourceModPtr)]
pub struct ISourceModVtable {
    // SMInterface
    pub GetInterfaceVersion: fn() -> c_uint,
    pub GetInterfaceName: fn() -> *const c_char,
    pub IsVersionCompatible: fn(version: c_uint) -> bool,

    // ISourceMod
    pub GetGamePath: fn() -> *const c_char,
    pub GetSourceModPath: fn() -> *const c_char,
    pub BuildPath: fn(ty: PathType, buffer: *mut c_char, maxlength: size_t, format: *const c_char, ...) -> size_t,
    pub LogMessage: fn(ext: IExtensionPtr, format: *const c_char, ...) -> (),
    pub LogError: fn(ext: IExtensionPtr, format: *const c_char, ...) -> (),
    pub FormatString: fn(buffer: *mut c_char, maxlength: size_t, context: IPluginContextPtr, params: *const cell_t, param: c_uint) -> size_t,
    _CreateDataPack: fn(),
    _FreeDataPack: fn(),
    _GetDataPackHandleType: fn(),
    _ReadKeyValuesHandle: fn(),
    pub GetGameFolderName: fn() -> *const c_char,
    pub GetScriptingEngine: fn() -> *mut c_void,
    pub GetScriptingVM: fn() -> *mut c_void,
    _GetAdjustedTime: fn(),
    pub SetGlobalTarget: fn(index: c_uint) -> c_uint,
    pub GetGlobalTarget: fn() -> c_uint,
    pub AddGameFrameHook: fn(hook: GameFrameHookFunc) -> (),
    pub RemoveGameFrameHook: fn(hook: GameFrameHookFunc) -> (),
    pub Format: fn(buffer: *mut c_char, maxlength: size_t, format: *const c_char, ...) -> size_t,
    _FormatArgs: fn(),
    pub AddFrameAction: fn(func: unsafe extern "C" fn(*mut c_void), data: *mut c_void) -> (),
    pub GetCoreConfigValue: fn(key: *const c_char) -> *const c_char,
    pub GetPluginId: fn() -> c_int,
    pub GetShApiVersion: fn() -> c_int,
    pub IsMapRunning: fn() -> bool,
    pub FromPseudoAddress: fn(pseudo: u32) -> *mut c_void,
    pub ToPseudoAddress: fn(addr: *mut c_void) -> u32,
}

#[derive(Debug, SMInterfaceApi)]
#[interface("ISourceMod", 14)]
pub struct ISourceMod(pub(crate) ISourceModPtr);

pub struct GameFrameHookId(pub(crate) GameFrameHookFunc, pub(crate) ISourceModPtr);

impl Drop for GameFrameHookId {
    fn drop(&mut self) {
        ISourceMod(self.1).remove_game_frame_hook(self.0);
    }
}

unsafe extern "C" fn frame_action_trampoline<F: FnMut() + 'static>(func: *mut c_void) {
    let mut func: Box<F> = Box::from_raw(func as *mut _);
    (*func)()
}

impl ISourceMod {
    pub fn log_message(&self, myself: &IExtension, msg: String) {
        let fmt = c_str_macro::c_str!("%s");
        let msg = CString::new(msg).expect("log message contained NUL byte");
        unsafe { virtual_call_varargs!(LogMessage, self.0, myself.0, fmt.as_ptr(), msg.as_ptr()) }
    }

    pub fn log_error(&self, myself: &IExtension, msg: String) {
        let fmt = c_str_macro::c_str!("%s");
        let msg = CString::new(msg).expect("log message contained NUL byte");
        unsafe { virtual_call_varargs!(LogError, self.0, myself.0, fmt.as_ptr(), msg.as_ptr()) }
    }

    /// Add a function that will be called every game frame until the [`GameFrameHookId`] return value
    /// is dropped. This is a fairly low-level building block as the callback must be `extern "C"`.
    pub fn add_game_frame_hook(&self, hook: GameFrameHookFunc) -> GameFrameHookId {
        unsafe {
            virtual_call!(AddGameFrameHook, self.0, hook);
        }

        GameFrameHookId(hook, self.0)
    }

    fn remove_game_frame_hook(&self, hook: GameFrameHookFunc) {
        unsafe {
            virtual_call!(RemoveGameFrameHook, self.0, hook);
        }
    }

    // TODO: If we implement a [`Send`] subset of [`ISourceMod`] this function should be included but the closure must also be [`Send`].
    /// Add a function that will be called on the next game frame. This has a runtime cost as this API
    /// is thread-safe on the SM side, but it supports a Rust closure so is more flexible than [`ISourceMod::add_game_frame_hook`].
    pub fn add_frame_action<F>(&self, func: F)
    where
        F: FnMut() + 'static,
    {
        unsafe {
            let func = Box::into_raw(Box::new(func));
            virtual_call!(AddFrameAction, self.0, frame_action_trampoline::<F>, func as *mut c_void);
        }
    }
}
