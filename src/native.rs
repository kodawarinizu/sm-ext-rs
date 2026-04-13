use std::error::Error;

use crate::plugin::{IPluginContext, IPluginContextPtr};
use crate::types::{cell_t, TryIntoPlugin};

// ------------------------------------------------------------------------------------------------
// NativeResult
// ------------------------------------------------------------------------------------------------

/// The return type for native callbacks.
pub trait NativeResult {
    type Ok;
    type Err;

    fn into_result(self) -> Result<Self::Ok, Self::Err>;
}

/// Dummy error used for [`NativeResult`] implementations that can never fail.
#[derive(Debug)]
pub struct DummyNativeError;

impl std::fmt::Display for DummyNativeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        std::fmt::Debug::fmt(self, f)
    }
}

impl Error for DummyNativeError {}

impl NativeResult for () {
    type Ok = i32;
    type Err = DummyNativeError;

    fn into_result(self) -> Result<Self::Ok, Self::Err> {
        Ok(0)
    }
}

impl<'ctx, T> NativeResult for T
where
    T: TryIntoPlugin<'ctx, cell_t>,
{
    type Ok = T;
    type Err = DummyNativeError;

    fn into_result(self) -> Result<Self::Ok, Self::Err> {
        Ok(self)
    }
}

impl<E> NativeResult for Result<(), E> {
    type Ok = i32;
    type Err = E;

    #[allow(clippy::type_complexity)]
    fn into_result(self) -> Result<<Result<(), E> as NativeResult>::Ok, <Result<(), E> as NativeResult>::Err> {
        self.map(|_| 0)
    }
}

impl<'ctx, T, E> NativeResult for Result<T, E>
where
    T: TryIntoPlugin<'ctx, cell_t>,
{
    type Ok = T;
    type Err = E;

    #[allow(clippy::type_complexity)]
    fn into_result(self) -> Result<<Result<T, E> as NativeResult>::Ok, <Result<T, E> as NativeResult>::Err> {
        self
    }
}

// ------------------------------------------------------------------------------------------------
// safe_native_invoke
// ------------------------------------------------------------------------------------------------

/// Wrapper to invoke a native callback and translate a [`panic!`] or [`Err`](std::result::Result::Err)
/// return into a SourceMod error using [`IPluginContext::throw_native_error`].
///
/// This is used internally by the `#[native]` attribute.
pub fn safe_native_invoke<F>(ctx: IPluginContextPtr, f: F) -> cell_t
where
    F: FnOnce(&IPluginContext) -> Result<cell_t, Box<dyn Error>> + std::panic::UnwindSafe,
{
    let ctx = IPluginContext(ctx);
    let result = std::panic::catch_unwind(|| f(&ctx));

    match result {
        Ok(result) => match result {
            Ok(result) => result,
            Err(err) => ctx.throw_native_error(err.to_string()),
        },
        Err(err) => {
            let msg = format!(
                "native panicked: {}",
                if let Some(str_slice) = err.downcast_ref::<&'static str>() {
                    str_slice
                } else if let Some(string) = err.downcast_ref::<String>() {
                    string
                } else {
                    "unknown message"
                }
            );

            ctx.throw_native_error(msg)
        }
    }
}
