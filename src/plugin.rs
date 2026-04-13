use sm_ext_derive::{vtable, ICallableApi};

use std::error::Error;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr::null_mut;

use crate::callable::{Executable, ICallableApi, IPluginFunctionPtr};
use crate::types::{cell_t, SPError, TryFromPlugin, TryIntoPlugin};
use crate::{virtual_call, virtual_call_varargs};

// ------------------------------------------------------------------------------------------------
// IPluginContext
// ------------------------------------------------------------------------------------------------

pub type IPluginContextPtr = *mut *mut IPluginContextVtable;

#[vtable(IPluginContextPtr)]
pub struct IPluginContextVtable {
    _Destructor: fn() -> (),
    #[cfg(not(windows))]
    _Destructor2: fn() -> (),
    _GetVirtualMachine: fn(),
    _GetContext: fn(),
    _IsDebugging: fn(),
    _SetDebugBreak: fn(),
    _GetDebugInfo: fn(),
    _HeapAlloc: fn(),
    _HeapPop: fn(),
    _HeapRelease: fn(),
    _FindNativeByName: fn(),
    _GetNativeByIndex: fn(),
    _GetNativesNum: fn(),
    _FindPublicByName: fn(),
    _GetPublicByIndex: fn(),
    _GetPublicsNum: fn(),
    _GetPubvarByIndex: fn(),
    _FindPubvarByName: fn(),
    _GetPubvarAddrs: fn(),
    _GetPubVarsNum: fn(),
    pub LocalToPhysAddr: fn(local_addr: cell_t, phys_addr: *mut *mut cell_t) -> SPError,
    pub LocalToString: fn(local_addr: cell_t, addr: *mut *mut c_char) -> SPError,
    _StringToLocal: fn(),
    _StringToLocalUTF8: fn(),
    _PushCell: fn(),
    _PushCellArray: fn(),
    _PushString: fn(),
    _PushCellsFromArray: fn(),
    _BindNatives: fn(),
    _BindNative: fn(),
    _BindNativeToAny: fn(),
    _Execute: fn(),
    _ThrowNativeErrorEx: fn(),
    pub ThrowNativeError: fn(*const c_char, ...) -> cell_t,
    pub GetFunctionByName: fn(public_name: *const c_char) -> IPluginFunctionPtr,
    pub GetFunctionById: fn(func_id: u32) -> IPluginFunctionPtr,
    pub GetIdentity: fn() -> crate::IdentityTokenPtr,
    _GetNullRef: fn(),
    _LocalToStringNULL: fn(),
    _BindNativeToIndex: fn(),
    _IsInExec: fn(),
    _GetRuntime: fn(),
    _Execute2: fn(),
    _GetLastNativeError: fn(),
    _GetLocalParams: fn(),
    _SetKey: fn(),
    _GetKey: fn(),
    _ClearLastNativeError: fn(),
    _APIv2: fn(),
    _ReportError: fn(),
    _ReportErrorVA: fn(),
    _ReportFatalError: fn(),
    _ReportFatalErrorVA: fn(),
    _ReportErrorNumber: fn(),
    _BlamePluginError: fn(),
    _CreateFrameIterator: fn(),
    _DestroyFrameIterator: fn(),
}

#[derive(Debug)]
pub enum GetFunctionError {
    UnknownFunction,
}

impl std::fmt::Display for GetFunctionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        std::fmt::Debug::fmt(self, f)
    }
}

impl Error for GetFunctionError {}

#[derive(Debug)]
pub struct IPluginContext(pub(crate) IPluginContextPtr);

impl IPluginContext {
    pub fn local_to_phys_addr(&self, local: cell_t) -> Result<&mut cell_t, SPError> {
        unsafe {
            let mut addr: *mut cell_t = null_mut();
            let res = virtual_call!(LocalToPhysAddr, self.0, local, &mut addr);

            match res {
                SPError::None => Ok(&mut *addr),
                _ => Err(res),
            }
        }
    }

    pub fn local_to_string(&self, local: cell_t) -> Result<&CStr, SPError> {
        unsafe {
            let mut addr: *mut c_char = null_mut();
            let res = virtual_call!(LocalToString, self.0, local, &mut addr);

            match res {
                SPError::None => Ok(CStr::from_ptr(addr)),
                _ => Err(res),
            }
        }
    }

    pub fn throw_native_error(&self, err: String) -> cell_t {
        let fmt = c_str_macro::c_str!("%s");
        let err = std::ffi::CString::new(err).unwrap_or_else(|_| c_str_macro::c_str!("native error message contained NUL byte").into());
        unsafe { virtual_call_varargs!(ThrowNativeError, self.0, fmt.as_ptr(), err.as_ptr()) }
    }

    pub fn get_function_by_id(&self, func_id: u32) -> Result<IPluginFunction<'_>, GetFunctionError> {
        unsafe {
            let function = virtual_call!(GetFunctionById, self.0, func_id);
            if function.is_null() {
                Err(GetFunctionError::UnknownFunction)
            } else {
                Ok(IPluginFunction(function, self))
            }
        }
    }

    pub fn get_identity(&self) -> crate::IdentityTokenPtr {
        unsafe { virtual_call!(GetIdentity, self.0) }
    }
}

// ------------------------------------------------------------------------------------------------
// TryFromPlugin impls that require IPluginContext
// ------------------------------------------------------------------------------------------------

impl<'ctx> TryFromPlugin<'ctx> for &'ctx CStr {
    type Error = SPError;

    fn try_from_plugin(ctx: &'ctx IPluginContext, value: cell_t) -> Result<Self, Self::Error> {
        Ok(ctx.local_to_string(value)?)
    }
}

impl<'ctx> TryFromPlugin<'ctx> for &'ctx str {
    type Error = Box<dyn Error>;

    fn try_from_plugin(ctx: &'ctx IPluginContext, value: cell_t) -> Result<Self, Self::Error> {
        Ok(ctx.local_to_string(value)?.to_str()?)
    }
}

// TODO: These &mut implementations seem risky, maybe a SPRef/SPString/SPArray wrapper object would be a better way to go...

impl<'ctx> TryFromPlugin<'ctx> for &'ctx mut cell_t {
    type Error = SPError;

    fn try_from_plugin(ctx: &'ctx IPluginContext, value: cell_t) -> Result<Self, Self::Error> {
        Ok(ctx.local_to_phys_addr(value)?)
    }
}

impl<'ctx> TryFromPlugin<'ctx> for &'ctx mut i32 {
    type Error = SPError;

    fn try_from_plugin(ctx: &'ctx IPluginContext, value: cell_t) -> Result<Self, Self::Error> {
        let cell: &mut cell_t = value.try_into_plugin(ctx)?;
        unsafe { Ok(&mut *(cell as *mut cell_t as *mut i32)) }
    }
}

impl<'ctx> TryFromPlugin<'ctx> for &'ctx mut f32 {
    type Error = SPError;

    fn try_from_plugin(ctx: &'ctx IPluginContext, value: cell_t) -> Result<Self, Self::Error> {
        let cell: &mut cell_t = value.try_into_plugin(ctx)?;
        unsafe { Ok(&mut *(cell as *mut cell_t as *mut f32)) }
    }
}

// ------------------------------------------------------------------------------------------------
// IPluginFunction
// ------------------------------------------------------------------------------------------------

#[derive(Debug, ICallableApi)]
pub struct IPluginFunction<'ctx>(pub(crate) IPluginFunctionPtr, #[allow(dead_code)] pub(crate) &'ctx IPluginContext);

impl Executable for IPluginFunction<'_> {
    fn execute(&mut self) -> Result<cell_t, SPError> {
        unsafe {
            let mut result: cell_t = 0.into();
            let res = virtual_call!(Execute, self.0, &mut result);
            match res {
                SPError::None => Ok(result),
                _ => Err(res),
            }
        }
    }
}

impl<'ctx> TryFromPlugin<'ctx> for IPluginFunction<'ctx> {
    type Error = GetFunctionError;

    fn try_from_plugin(ctx: &'ctx IPluginContext, value: cell_t) -> Result<Self, Self::Error> {
        ctx.get_function_by_id(value.0 as u32)
    }
}
