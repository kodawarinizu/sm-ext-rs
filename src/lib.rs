#![cfg_attr(feature = "abi_thiscall", feature(abi_thiscall))]
#![allow(non_snake_case, non_camel_case_types, unused_variables)]

mod callable;
mod extension;
mod handle;
mod native;
mod plugin;
mod sharedsys;
mod sourcemod;
mod types;

pub use callable::*;
pub use extension::*;
pub use handle::*;
pub use native::*;
pub use plugin::*;
pub use sharedsys::*;
pub use sourcemod::*;
pub use types::*;

pub use c_str_macro::c_str;
pub use libc::size_t;
pub use sm_ext_derive::{forwards, native, vtable, vtable_override, ICallableApi, SMExtension, SMInterfaceApi};

// ------------------------------------------------------------------------------------------------
// Macros
// ------------------------------------------------------------------------------------------------

/// Helper for virtual function invocation that works with the `#[vtable]` attribute to support
/// virtual calls on Windows without compiler support for the `thiscall` calling convention.
#[macro_export]
macro_rules! virtual_call {
    ($name:ident, $this:expr, $($param:expr),* $(,)?) => {
        ((**$this).$name)(
            $this,
            #[cfg(all(windows, target_arch = "x86", not(feature = "abi_thiscall")))]
            std::ptr::null_mut(),
            $(
                $param,
            )*
        )
    };
    ($name:ident, $this:expr) => {
        virtual_call!($name, $this, )
    };
}

// TODO: Figure out a way to make this type-safe (and hopefully avoid the need for it completely.)
/// Helper for varargs-using virtual function invocation that works with the `#[vtable]` attribute to
/// support virtual calls on Windows without compiler support for the `thiscall` calling convention.
#[macro_export]
macro_rules! virtual_call_varargs {
    ($name:ident, $this:expr, $($param:expr),* $(,)?) => {
        ((**$this).$name)(
            $this,
            $(
                $param,
            )*
        )
    };
    ($name:ident, $this:expr) => {
        virtual_call!($name, $this, )
    };
}

#[macro_export]
macro_rules! register_natives {
    ($sys:expr, $myself:expr, [$(($name:expr, $func:expr)),* $(,)?]) => {
        unsafe {
            let mut vec = Vec::new();
            $(
                let name = concat!($name, "\0").as_ptr() as *const ::std::os::raw::c_char;
                vec.push($crate::NativeInfo {
                    name: name,
                    func: Some($func),
                });
            )*
            vec.push($crate::NativeInfo {
                name: ::std::ptr::null(),
                func: None,
            });

            // This leaks vec so that it remains valid.
            // TODO: Look into making it static somewhere, it only has to live as long as the extension is loaded.
            // Would probably need some of the nightly macro features, which tbh would help the native callbacks anyway.
            let boxed = vec.into_boxed_slice();
            $sys.add_natives($myself, Box::leak(boxed).as_ptr());
        }
    };
}
