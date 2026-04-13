use std::error::Error;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::ptr::{null, null_mut};
use libc::size_t;

use sm_ext_derive::{vtable, ICallableApi, SMInterfaceApi};

use crate::plugin::{IPluginContextPtr, IPluginFunction};
use crate::sharedsys::{RequestableInterface, SMInterface, SMInterfaceApi};
use crate::types::{cell_t, HandleId, SPError};
use crate::{virtual_call, virtual_call_varargs};

// ------------------------------------------------------------------------------------------------
// ExecType / ParamType
// ------------------------------------------------------------------------------------------------

/// Defines how a forward iterates through plugin functions.
#[repr(C)]
pub enum ExecType {
    /// Ignore all return values, return 0
    Ignore = 0,
    /// Only return the last exec, ignore all others
    Single = 1,
    /// Acts as an event with the ResultTypes above, no mid-Stops allowed, returns highest
    Event = 2,
    /// Acts as a hook with the ResultTypes above, mid-Stops allowed, returns highest
    Hook = 3,
    /// Same as Event except that it returns the lowest value
    LowEvent = 4,
}

/// Describes the various ways to pass parameters to plugins.
#[repr(C)]
pub enum ParamType {
    /// Any data type can be pushed
    Any = 0,
    /// Only basic cells can be pushed
    Cell = (1 << 1),
    /// Only floats can be pushed
    Float = (2 << 1),
    /// Only strings can be pushed
    String = (3 << 1) | 1,
    /// Only arrays can be pushed
    Array = (4 << 1) | 1,
    /// Same as "..." in plugins, anything can be pushed, but it will always be byref
    VarArgs = (5 << 1),
    /// Only a cell by reference can be pushed
    CellByRef = (1 << 1) | 1,
    /// Only a float by reference can be pushed
    FloatByRef = (2 << 1) | 1,
}

// ------------------------------------------------------------------------------------------------
// IPluginFunction vtable
// ------------------------------------------------------------------------------------------------

pub type IPluginFunctionPtr = *mut *mut IPluginFunctionVtable;

#[vtable(IPluginFunctionPtr)]
pub struct IPluginFunctionVtable {
    // ICallable
    pub PushCell: fn(cell: cell_t) -> SPError,
    pub PushCellByRef: fn(cell: *mut cell_t, flags: c_int) -> SPError,
    pub PushFloat: fn(number: f32) -> SPError,
    pub PushFloatByRef: fn(number: *mut f32, flags: c_int) -> SPError,
    pub PushArray: fn(cell: *mut cell_t, cells: c_uint, flags: c_int) -> SPError,
    pub PushString: fn(string: *const c_char) -> SPError,
    pub PushStringEx: fn(string: *const c_char, length: size_t, sz_flags: c_int, cp_flags: c_int) -> SPError,
    pub Cancel: fn(),

    // IPluginFunction
    pub Execute: fn(result: *mut cell_t) -> SPError,
    _CallFunction: fn(),
    _GetParentContext: fn(),
    pub IsRunnable: fn() -> bool,
    pub GetFunctionID: fn() -> u32,
    _Execute2: fn(),
    _CallFunction2: fn(),
    _GetParentRuntime: fn(),
    pub Invoke: fn(rval: *mut cell_t) -> bool,
    pub DebugName: fn() -> *const c_char,
}

// ------------------------------------------------------------------------------------------------
// Forward vtables
// ------------------------------------------------------------------------------------------------

pub type IForwardPtr = *mut *mut IForwardVtable;

#[vtable(IForwardPtr)]
pub struct IForwardVtable {
    // ICallable
    pub PushCell: fn(cell: cell_t) -> SPError,
    pub PushCellByRef: fn(cell: *mut cell_t, flags: c_int) -> SPError,
    pub PushFloat: fn(number: f32) -> SPError,
    pub PushFloatByRef: fn(number: *mut f32, flags: c_int) -> SPError,
    pub PushArray: fn(cell: *mut cell_t, cells: c_uint, flags: c_int) -> SPError,
    pub PushString: fn(string: *const c_char) -> SPError,
    pub PushStringEx: fn(string: *const c_char, length: size_t, sz_flags: c_int, cp_flags: c_int) -> SPError,
    pub Cancel: fn(),

    // IForward
    _Destructor: fn() -> (),
    #[cfg(not(windows))]
    _Destructor2: fn() -> (),
    pub GetForwardName: fn() -> *const c_char,
    pub GetFunctionCount: fn() -> c_uint,
    pub GetExecType: fn() -> ExecType,
    pub Execute: fn(result: *mut cell_t, filter: *mut c_void) -> SPError,
}

pub type IChangeableForwardPtr = *mut *mut IChangeableForwardVtable;

#[vtable(IChangeableForwardPtr)]
pub struct IChangeableForwardVtable {
    // ICallable
    pub PushCell: fn(cell: cell_t) -> SPError,
    pub PushCellByRef: fn(cell: *mut cell_t, flags: c_int) -> SPError,
    pub PushFloat: fn(number: f32) -> SPError,
    pub PushFloatByRef: fn(number: *mut f32, flags: c_int) -> SPError,
    pub PushArray: fn(cell: *mut cell_t, cells: c_uint, flags: c_int) -> SPError,
    pub PushString: fn(string: *const c_char) -> SPError,
    pub PushStringEx: fn(string: *const c_char, length: size_t, sz_flags: c_int, cp_flags: c_int) -> SPError,
    pub Cancel: fn(),

    // IForward
    _Destructor: fn() -> (),
    #[cfg(not(windows))]
    _Destructor2: fn() -> (),
    pub GetForwardName: fn() -> *const c_char,
    pub GetFunctionCount: fn() -> c_uint,
    pub GetExecType: fn() -> ExecType,
    pub Execute: fn(result: *mut cell_t, filter: *mut c_void) -> SPError,

    // IChangeableForward
    #[cfg(windows)]
    pub RemoveFunctionById: fn(ctx: IPluginContextPtr, func: u32) -> bool,
    pub RemoveFunction: fn(func: IPluginFunctionPtr) -> bool,
    _RemoveFunctionsOfPlugin: fn(),
    #[cfg(windows)]
    pub AddFunctionById: fn(ctx: IPluginContextPtr, func: u32) -> bool,
    pub AddFunction: fn(func: IPluginFunctionPtr) -> bool,
    #[cfg(not(windows))]
    pub AddFunctionById: fn(ctx: IPluginContextPtr, func: u32) -> bool,
    #[cfg(not(windows))]
    pub RemoveFunctionById: fn(ctx: IPluginContextPtr, func: u32) -> bool,
}

// ------------------------------------------------------------------------------------------------
// CallableParam / ICallableApi / Executable
// ------------------------------------------------------------------------------------------------

pub trait CallableParam {
    fn push<T: ICallableApi>(&self, callable: &mut T) -> Result<(), SPError>;
    fn param_type() -> ParamType;
}

impl CallableParam for cell_t {
    fn push<T: ICallableApi>(&self, callable: &mut T) -> Result<(), SPError> {
        callable.push_int(self.0)
    }

    fn param_type() -> ParamType {
        ParamType::Cell
    }
}

impl CallableParam for i32 {
    fn push<T: ICallableApi>(&self, callable: &mut T) -> Result<(), SPError> {
        callable.push_int(*self)
    }

    fn param_type() -> ParamType {
        ParamType::Cell
    }
}

impl CallableParam for f32 {
    fn push<T: ICallableApi>(&self, callable: &mut T) -> Result<(), SPError> {
        callable.push_float(*self)
    }

    fn param_type() -> ParamType {
        ParamType::Float
    }
}

impl CallableParam for &CStr {
    fn push<T: ICallableApi>(&self, callable: &mut T) -> Result<(), SPError> {
        callable.push_string(self)
    }

    fn param_type() -> ParamType {
        ParamType::String
    }
}

impl CallableParam for HandleId {
    fn push<T: ICallableApi>(&self, callable: &mut T) -> Result<(), SPError> {
        callable.push_int(self.0 as i32)
    }

    fn param_type() -> ParamType {
        ParamType::Cell
    }
}

// TODO: This interface is very, very rough.
pub trait ICallableApi {
    fn push_int(&mut self, cell: i32) -> Result<(), SPError>;
    fn push_float(&mut self, number: f32) -> Result<(), SPError>;
    fn push_string(&mut self, string: &CStr) -> Result<(), SPError>;
}

pub trait Executable: ICallableApi + Sized {
    fn execute(&mut self) -> Result<cell_t, SPError>;

    fn push<T: CallableParam>(&mut self, param: T) -> Result<(), SPError> {
        param.push(self)
    }
}

// ------------------------------------------------------------------------------------------------
// Forward / ChangeableForward
// ------------------------------------------------------------------------------------------------

pub type IForwardManagerPtr = *mut *mut IForwardManagerVtable;

#[derive(Debug, ICallableApi)]
pub struct Forward(pub(crate) IForwardPtr, pub(crate) IForwardManagerPtr);

impl Drop for Forward {
    fn drop(&mut self) {
        IForwardManager(self.1).release_forward(&mut self.0);
    }
}

impl Executable for Forward {
    fn execute(&mut self) -> Result<cell_t, SPError> {
        unsafe {
            let mut result: cell_t = 0.into();
            let res = virtual_call!(Execute, self.0, &mut result, null_mut());
            match res {
                SPError::None => Ok(result),
                _ => Err(res),
            }
        }
    }
}

impl Forward {
    pub fn get_function_count(&self) -> u32 {
        unsafe { virtual_call!(GetFunctionCount, self.0) }
    }
}

#[derive(Debug, ICallableApi)]
pub struct ChangeableForward(pub(crate) IChangeableForwardPtr, pub(crate) IForwardManagerPtr);

impl Drop for ChangeableForward {
    fn drop(&mut self) {
        IForwardManager(self.1).release_forward(&mut (self.0 as IForwardPtr));
    }
}

impl Executable for ChangeableForward {
    fn execute(&mut self) -> Result<cell_t, SPError> {
        unsafe {
            let mut result: cell_t = 0.into();
            let res = virtual_call!(Execute, self.0, &mut result, null_mut());
            match res {
                SPError::None => Ok(result),
                _ => Err(res),
            }
        }
    }
}

impl ChangeableForward {
    pub fn get_function_count(&self) -> u32 {
        unsafe { virtual_call!(GetFunctionCount, self.0) }
    }

    pub fn add_function(&mut self, func: &mut IPluginFunction) {
        unsafe {
            virtual_call!(AddFunction, self.0, func.0);
        }
    }

    pub fn remove_function(&mut self, func: &mut IPluginFunction) {
        unsafe {
            virtual_call!(RemoveFunction, self.0, func.0);
        }
    }
}

// ------------------------------------------------------------------------------------------------
// IForwardManager
// ------------------------------------------------------------------------------------------------

#[vtable(IForwardManagerPtr)]
pub struct IForwardManagerVtable {
    // SMInterface
    pub GetInterfaceVersion: fn() -> c_uint,
    pub GetInterfaceName: fn() -> *const c_char,
    pub IsVersionCompatible: fn(version: c_uint) -> bool,

    // IForwardManager
    pub CreateForward: fn(name: *const c_char, et: ExecType, num_params: c_uint, types: *const ParamType, ...) -> IForwardPtr,
    pub CreateForwardEx: fn(name: *const c_char, et: ExecType, num_params: c_uint, types: *const ParamType, ...) -> IChangeableForwardPtr,
    pub FindForward: fn(name: *const c_char, *mut IChangeableForwardPtr) -> IForwardPtr,
    pub ReleaseForward: fn(forward: IForwardPtr) -> (),
}

#[derive(Debug)]
pub enum CreateForwardError {
    InvalidName(std::ffi::NulError),
    InvalidParams(Option<String>),
}

impl std::fmt::Display for CreateForwardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            CreateForwardError::InvalidName(err) => write!(f, "invalid forward name: {}", err),
            CreateForwardError::InvalidParams(name) => match name {
                Some(name) => write!(f, "failed to create forward {}: invalid params", name),
                None => write!(f, "failed to create forward anonymous forward: invalid params"),
            },
        }
    }
}

impl Error for CreateForwardError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CreateForwardError::InvalidName(err) => Some(err),
            CreateForwardError::InvalidParams(_) => None,
        }
    }
}

#[derive(Debug, SMInterfaceApi)]
#[interface("IForwardManager", 4)]
pub struct IForwardManager(pub(crate) IForwardManagerPtr);

impl IForwardManager {
    pub fn create_global_forward(&self, name: &str, et: ExecType, params: &[ParamType]) -> Result<Forward, CreateForwardError> {
        let c_name = CString::new(name).map_err(CreateForwardError::InvalidName)?;

        unsafe {
            let forward = virtual_call_varargs!(CreateForward, self.0, c_name.as_ptr(), et, params.len() as u32, params.as_ptr());

            if forward.is_null() {
                Err(CreateForwardError::InvalidParams(Some(name.into())))
            } else {
                Ok(Forward(forward, self.0))
            }
        }
    }

    pub fn create_private_forward(&self, name: Option<&str>, et: ExecType, params: &[ParamType]) -> Result<ChangeableForward, CreateForwardError> {
        let c_name = match name {
            Some(name) => Some(CString::new(name).map_err(CreateForwardError::InvalidName)?),
            None => None,
        };

        let c_name = match c_name {
            Some(c_name) => c_name.as_ptr(),
            None => null(),
        };

        unsafe {
            let forward = virtual_call_varargs!(CreateForwardEx, self.0, c_name, et, params.len() as u32, params.as_ptr());

            if forward.is_null() {
                Err(CreateForwardError::InvalidParams(name.map(|name| name.into())))
            } else {
                Ok(ChangeableForward(forward, self.0))
            }
        }
    }

    fn release_forward(&self, forward: &mut IForwardPtr) {
        if forward.is_null() {
            panic!("release_forward called on null forward ptr")
        }

        unsafe {
            virtual_call!(ReleaseForward, self.0, *forward);
            *forward = null_mut();
        }
    }
}
