use sm_ext_derive::{vtable, vtable_override, SMInterfaceApi};

use std::error::Error;
use std::ffi::CString;
use std::os::raw::{c_char, c_uint, c_void};
use std::ptr::null;
use std::rc::Rc;

use crate::sharedsys::{RequestableInterface, SMInterface, SMInterfaceApi};
use crate::types::{HandleId, HandleTypeId, IdentityTokenPtr};
use crate::virtual_call;

// ------------------------------------------------------------------------------------------------
// HandleError
// ------------------------------------------------------------------------------------------------

/// Lists the possible handle error codes.
#[repr(C)]
#[derive(Debug)]
pub enum HandleError {
    /// No error
    None = 0,
    /// The handle has been freed and reassigned
    Changed = 1,
    /// The handle has a different type registered
    Type = 2,
    /// The handle has been freed
    Freed = 3,
    /// Generic internal indexing error
    Index = 4,
    /// No access permitted to free this handle
    Access = 5,
    /// The limited number of handles has been reached
    Limit = 6,
    /// The identity token was not usable
    Identity = 7,
    /// Owners do not match for this operation
    Owner = 8,
    /// Unrecognized security structure version
    Version = 9,
    /// An invalid parameter was passed
    Parameter = 10,
    /// This type cannot be inherited
    NoInherit = 11,
}

impl std::fmt::Display for HandleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.pad(match self {
            HandleError::None => "no error",
            HandleError::Changed => "the handle has been freed and reassigned",
            HandleError::Type => "the handle has a different type registered",
            HandleError::Freed => "the handle has been freed",
            HandleError::Index => "generic internal indexing error",
            HandleError::Access => "no access permitted to free this handle",
            HandleError::Limit => "the limited number of handles has been reached",
            HandleError::Identity => "the identity token was not usable",
            HandleError::Owner => "owners do not match for this operation",
            HandleError::Version => "unrecognized security structure version",
            HandleError::Parameter => "an invalid parameter was passed",
            HandleError::NoInherit => "this type cannot be inherited",
        })
    }
}

impl Error for HandleError {}

// ------------------------------------------------------------------------------------------------
// IHandleTypeDispatch vtable + adapter
// ------------------------------------------------------------------------------------------------

pub type IHandleTypeDispatchPtr = *mut *mut IHandleTypeDispatchVtable;

#[vtable(IHandleTypeDispatchPtr)]
pub struct IHandleTypeDispatchVtable {
    pub GetDispatchVersion: fn() -> c_uint,
    pub OnHandleDestroy: fn(ty: HandleTypeId, object: *mut c_void) -> (),
    pub GetHandleApproxSize: fn(ty: HandleTypeId, object: *mut c_void, size: *mut c_uint) -> bool,
}

#[repr(C)]
pub struct IHandleTypeDispatchAdapter<T> {
    vtable: *mut IHandleTypeDispatchVtable,
    phantom: std::marker::PhantomData<T>,
}

impl<T> Drop for IHandleTypeDispatchAdapter<T> {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.vtable));
        }
    }
}

impl<T> Default for IHandleTypeDispatchAdapter<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> IHandleTypeDispatchAdapter<T> {
    pub fn new() -> IHandleTypeDispatchAdapter<T> {
        let vtable = IHandleTypeDispatchVtable {
            GetDispatchVersion: IHandleTypeDispatchAdapter::<T>::get_dispatch_version,
            OnHandleDestroy: IHandleTypeDispatchAdapter::<T>::on_handle_destroy,
            GetHandleApproxSize: IHandleTypeDispatchAdapter::<T>::get_handle_approx_size,
        };

        IHandleTypeDispatchAdapter { vtable: Box::into_raw(Box::new(vtable)), phantom: std::marker::PhantomData }
    }

    #[vtable_override]
    unsafe fn get_dispatch_version(this: IHandleTypeDispatchPtr) -> u32 {
        <IHandleSys as RequestableInterface>::get_interface_version()
    }

    #[vtable_override]
    unsafe fn on_handle_destroy(this: IHandleTypeDispatchPtr, ty: HandleTypeId, object: *mut c_void) {
        drop(Rc::from_raw(object as *mut T));
    }

    #[vtable_override]
    unsafe fn get_handle_approx_size(this: IHandleTypeDispatchPtr, ty: HandleTypeId, object: *mut c_void, size: *mut c_uint) -> bool {
        // This isn't ideal as it doesn't account for dynamic sizes, probably need to add a trait at some point
        // for people to implement this properly. See also: https://github.com/rust-lang/rust/issues/63073
        // This also isn't accounting for the Rc overhead as we're dealing with the internal ptr only.
        let object = object as *mut T;
        *size = std::mem::size_of_val(&*object) as u32;

        *size != 0
    }
}

// ------------------------------------------------------------------------------------------------
// HandleSecurity / HandleAccess
// ------------------------------------------------------------------------------------------------

/// This pair of tokens is used for identification.
#[repr(C)]
#[derive(Debug)]
pub struct HandleSecurity {
    /// Owner of the Handle
    pub owner: IdentityTokenPtr,
    /// Owner of the Type
    pub identity: IdentityTokenPtr,
}

impl HandleSecurity {
    pub fn new(owner: IdentityTokenPtr, identity: IdentityTokenPtr) -> Self {
        Self { owner, identity }
    }
}

#[repr(C)]
pub enum HandleAccessRestriction {
    Any = 0,
    IdentityOnly = 1,
    OwnerOnly = 2,
    OwnerAndIdentity = 3,
}

#[repr(C)]
pub struct HandleAccess {
    version: u32,
    pub read_access: HandleAccessRestriction,
    pub delete_access: HandleAccessRestriction,
    pub clone_access: HandleAccessRestriction,
}

impl HandleAccess {
    pub fn new() -> Self {
        HandleAccess {
            version: <IHandleSys as RequestableInterface>::get_interface_version(),
            read_access: HandleAccessRestriction::IdentityOnly,
            delete_access: HandleAccessRestriction::OwnerOnly,
            clone_access: HandleAccessRestriction::Any,
        }
    }
}

impl Default for HandleAccess {
    fn default() -> Self {
        Self::new()
    }
}

// ------------------------------------------------------------------------------------------------
// IHandleSys
// ------------------------------------------------------------------------------------------------

pub type IHandleSysPtr = *mut *mut IHandleSysVtable;

#[vtable(IHandleSysPtr)]
pub struct IHandleSysVtable {
    // SMInterface
    pub GetInterfaceVersion: fn() -> c_uint,
    pub GetInterfaceName: fn() -> *const c_char,
    pub IsVersionCompatible: fn(version: c_uint) -> bool,

    // IHandleSys
    pub CreateType: fn(name: *const c_char, dispatch: IHandleTypeDispatchPtr, parent: HandleTypeId, typeAccess: *const c_void, handleAccess: Option<&HandleAccess>, ident: IdentityTokenPtr, err: *mut HandleError) -> HandleTypeId,
    pub RemoveType: fn(ty: HandleTypeId, ident: IdentityTokenPtr) -> bool,
    pub FindHandleType: fn(name: *const c_char, ty: *mut HandleTypeId) -> bool,
    pub CreateHandle: fn(ty: HandleTypeId, object: *mut c_void, owner: IdentityTokenPtr, ident: IdentityTokenPtr, err: *mut HandleError) -> HandleId,
    pub FreeHandle: fn(handle: HandleId, security: *const HandleSecurity) -> HandleError,
    pub CloneHandle: fn(handle: HandleId, newHandle: *mut HandleId, newOwner: IdentityTokenPtr, security: *const HandleSecurity) -> HandleError,
    pub ReadHandle: fn(handle: HandleId, ty: HandleTypeId, security: *const HandleSecurity, object: *mut *mut c_void) -> HandleError,
    pub InitAccessDefaults: fn(typeAccess: *mut c_void, handleAccess: *mut c_void) -> bool,
    pub CreateHandleEx: fn(ty: HandleTypeId, object: *mut c_void, security: *const HandleSecurity, access: Option<&HandleAccess>, err: *mut HandleError) -> HandleId,
    pub FastCloneHandle: fn(handle: HandleId) -> HandleId,
    pub TypeCheck: fn(given: HandleTypeId, actual: HandleTypeId) -> bool,
}

#[derive(Debug)]
pub enum CreateHandleTypeError {
    InvalidName(std::ffi::NulError),
    HandleError(String, HandleError),
}

impl std::fmt::Display for CreateHandleTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            CreateHandleTypeError::InvalidName(err) => write!(f, "invalid handle type name: {}", err),
            CreateHandleTypeError::HandleError(name, err) => write!(f, "failed to create handle type {}: {}", name, err),
        }
    }
}

impl Error for CreateHandleTypeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CreateHandleTypeError::InvalidName(err) => Some(err),
            CreateHandleTypeError::HandleError(_, err) => Some(err),
        }
    }
}

#[derive(Debug, SMInterfaceApi)]
#[interface("IHandleSys", 5)]
pub struct IHandleSys(pub(crate) IHandleSysPtr);

impl IHandleSys {
    pub fn create_type<T>(&self, name: &str, handle_access: Option<&HandleAccess>, ident: IdentityTokenPtr) -> Result<HandleType<T>, CreateHandleTypeError> {
        unsafe {
            let c_name = CString::new(name).map_err(CreateHandleTypeError::InvalidName)?;
            let dispatch = Box::into_raw(Box::new(IHandleTypeDispatchAdapter::<T>::new()));

            let mut err: HandleError = HandleError::None;
            let id = virtual_call!(CreateType, self.0, c_name.as_ptr(), dispatch as IHandleTypeDispatchPtr, HandleTypeId::invalid(), null(), handle_access, ident, &mut err);

            if id.is_valid() {
                Ok(HandleType { iface: self.0, id, dispatch, ident })
            } else {
                Err(CreateHandleTypeError::HandleError(name.into(), err))
            }
        }
    }

    fn remove_type<T>(&self, ty: &mut HandleType<T>) -> Result<(), bool> {
        unsafe {
            if virtual_call!(RemoveType, self.0, ty.id, ty.ident) {
                Ok(())
            } else {
                Err(false)
            }
        }
    }

    fn create_handle<T>(&self, ty: &HandleType<T>, object: Rc<T>, owner: IdentityTokenPtr, access: Option<&HandleAccess>) -> Result<HandleId, HandleError> {
        unsafe {
            let object = Rc::into_raw(object) as *mut c_void;
            let security = HandleSecurity::new(owner, ty.ident);
            let mut err: HandleError = HandleError::None;
            let id = virtual_call!(CreateHandleEx, self.0, ty.id, object, &security, access, &mut err);
            if id.is_valid() {
                Ok(id)
            } else {
                Err(err)
            }
        }
    }

    fn free_handle<T>(&self, ty: &HandleType<T>, handle: HandleId, owner: IdentityTokenPtr) -> Result<(), HandleError> {
        unsafe {
            let security = HandleSecurity::new(owner, ty.ident);
            let err = virtual_call!(FreeHandle, self.0, handle, &security);
            match err {
                HandleError::None => Ok(()),
                _ => Err(err),
            }
        }
    }

    fn clone_handle<T>(&self, ty: &HandleType<T>, handle: HandleId, owner: IdentityTokenPtr, new_owner: IdentityTokenPtr) -> Result<HandleId, HandleError> {
        unsafe {
            let security = HandleSecurity::new(owner, ty.ident);
            let mut new_handle = HandleId::invalid();
            let err = virtual_call!(CloneHandle, self.0, handle, &mut new_handle, new_owner, &security);
            match err {
                HandleError::None => Ok(new_handle),
                _ => Err(err),
            }
        }
    }

    fn read_handle<T>(&self, ty: &HandleType<T>, handle: HandleId, owner: IdentityTokenPtr) -> Result<Rc<T>, HandleError> {
        unsafe {
            let security = HandleSecurity::new(owner, ty.ident);
            let mut object: *mut c_void = std::ptr::null_mut();
            let err = virtual_call!(ReadHandle, self.0, handle, ty.id, &security, &mut object);
            match err {
                HandleError::None => Ok({
                    // https://github.com/rust-lang/rust/issues/48108
                    let object = Rc::from_raw(object as *mut T);
                    std::mem::forget(object.clone());
                    object
                }),
                _ => Err(err),
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------
// HandleType<T>
// ------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct HandleType<T> {
    pub(crate) iface: IHandleSysPtr,
    pub(crate) id: HandleTypeId,
    pub(crate) dispatch: *mut IHandleTypeDispatchAdapter<T>,
    pub(crate) ident: IdentityTokenPtr,
}

impl<T> Drop for HandleType<T> {
    fn drop(&mut self) {
        IHandleSys(self.iface).remove_type(self).unwrap();

        unsafe {
            drop(Box::from_raw(self.dispatch));
        }
    }
}

impl<T> HandleType<T> {
    pub fn create_handle(&self, object: Rc<T>, owner: IdentityTokenPtr, access: Option<&HandleAccess>) -> Result<HandleId, HandleError> {
        IHandleSys(self.iface).create_handle(self, object, owner, access)
    }

    pub fn clone_handle(&self, handle: HandleId, owner: IdentityTokenPtr, new_owner: IdentityTokenPtr) -> Result<HandleId, HandleError> {
        IHandleSys(self.iface).clone_handle(self, handle, owner, new_owner)
    }

    pub fn free_handle(&self, handle: HandleId, owner: IdentityTokenPtr) -> Result<(), HandleError> {
        IHandleSys(self.iface).free_handle(self, handle, owner)
    }

    pub fn read_handle(&self, handle: HandleId, owner: IdentityTokenPtr) -> Result<Rc<T>, HandleError> {
        IHandleSys(self.iface).read_handle(self, handle, owner)
    }
}
