use std::convert::TryFrom;
use std::error::Error;
use std::os::raw::{c_char, c_uint};

// ------------------------------------------------------------------------------------------------
// IdentityType / FeatureType / FeatureStatus
// ------------------------------------------------------------------------------------------------

#[repr(transparent)]
pub struct IdentityType(c_uint);

#[repr(C)]
pub enum FeatureType {
    Native = 0,
    Capability = 1,
}

#[repr(C)]
pub enum FeatureStatus {
    Available = 0,
    Unavailable = 1,
    Unknown = 2,
}

// ------------------------------------------------------------------------------------------------
// cell_t
// ------------------------------------------------------------------------------------------------

// TODO: Investigate using a `union` for this instead.
/// Wrapper type that represents a value from SourcePawn.
///
/// Could be a [`i32`], [`f32`], `&i32`, `&f32`, or `&i8` (for character strings).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct cell_t(pub(crate) i32);

impl std::fmt::Display for cell_t {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.0.fmt(f)
    }
}

impl From<i32> for cell_t {
    fn from(x: i32) -> Self {
        cell_t(x)
    }
}

impl From<cell_t> for i32 {
    fn from(x: cell_t) -> Self {
        x.0
    }
}

impl From<f32> for cell_t {
    fn from(x: f32) -> Self {
        cell_t(x.to_bits() as i32)
    }
}

impl From<cell_t> for f32 {
    fn from(x: cell_t) -> Self {
        f32::from_bits(x.0 as u32)
    }
}

// ------------------------------------------------------------------------------------------------
// TryFromPlugin / TryIntoPlugin
// ------------------------------------------------------------------------------------------------

/// Trait to support conversions to/from [`cell_t`] that require an [`IPluginContext`](crate::IPluginContext) for access to plugin memory.
pub trait TryFromPlugin<'ctx, T = cell_t>: Sized {
    type Error;

    fn try_from_plugin(ctx: &'ctx crate::IPluginContext, value: T) -> Result<Self, Self::Error>;
}

impl<T, U> TryFromPlugin<'_, T> for U
where
    U: TryFrom<T>,
{
    type Error = U::Error;

    fn try_from_plugin(ctx: &crate::IPluginContext, value: T) -> Result<Self, Self::Error> {
        TryFrom::try_from(value)
    }
}

/// Trait to support conversions to/from [`cell_t`] that require an [`IPluginContext`](crate::IPluginContext) for access to plugin memory.
///
/// As with Rust's [`TryInto`](std::convert::TryInto) and [`TryFrom`](std::convert::TryFrom), this is implemented automatically
/// for types that implement [`TryFromPlugin`] which you should prefer to implement instead.
pub trait TryIntoPlugin<'ctx, T = cell_t>: Sized {
    type Error;

    fn try_into_plugin(self, ctx: &'ctx crate::IPluginContext) -> Result<T, Self::Error>;
}

impl<'ctx, T, U> TryIntoPlugin<'ctx, U> for T
where
    U: TryFromPlugin<'ctx, T>,
{
    type Error = U::Error;

    fn try_into_plugin(self, ctx: &'ctx crate::IPluginContext) -> Result<U, U::Error> {
        U::try_from_plugin(ctx, self)
    }
}

// ------------------------------------------------------------------------------------------------
// SPError
// ------------------------------------------------------------------------------------------------

/// Error codes for SourcePawn routines.
#[repr(C)]
#[derive(Debug)]
pub enum SPError {
    /// No error occurred
    None = 0,
    /// File format unrecognized
    FileFormat = 1,
    /// A decompressor was not found
    Decompressor = 2,
    /// Not enough space left on the heap
    HeapLow = 3,
    /// Invalid parameter or parameter type
    Param = 4,
    /// A memory address was not valid
    InvalidAddress = 5,
    /// The object in question was not found
    NotFound = 6,
    /// Invalid index parameter
    Index = 7,
    /// Not enough space left on the stack
    StackLow = 8,
    /// Debug mode was not on or debug section not found
    NotDebugging = 9,
    /// Invalid instruction was encountered
    InvalidInstruction = 10,
    /// Invalid memory access
    MemAccess = 11,
    /// Stack went beyond its minimum value
    StackMin = 12,
    /// Heap went beyond its minimum value
    HeapMin = 13,
    /// Division by zero
    DivideByZero = 14,
    /// Array index is out of bounds
    ArrayBounds = 15,
    /// Instruction had an invalid parameter
    InstructionParam = 16,
    /// A native leaked an item on the stack
    StackLeak = 17,
    /// A native leaked an item on the heap
    HeapLeak = 18,
    /// A dynamic array is too big
    ArrayTooBig = 19,
    /// Tracker stack is out of bounds
    TrackerBounds = 20,
    /// Native was pending or invalid
    InvalidNative = 21,
    /// Maximum number of parameters reached
    ParamsMax = 22,
    /// Error originates from a native
    Native = 23,
    /// Function or plugin is not runnable
    NotRunnable = 24,
    /// Function call was aborted
    Aborted = 25,
    /// Code is too old for this VM
    CodeTooOld = 26,
    /// Code is too new for this VM
    CodeTooNew = 27,
    /// Out of memory
    OutOfMemory = 28,
    /// Integer overflow (-INT_MIN / -1)
    IntegerOverflow = 29,
    /// Timeout
    Timeout = 30,
    /// Custom message
    User = 31,
    /// Custom fatal message
    Fatal = 32,
}

impl std::fmt::Display for SPError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.pad(match self {
            SPError::None => "no error occurred",
            SPError::FileFormat => "unrecognizable file format",
            SPError::Decompressor => "decompressor was not found",
            SPError::HeapLow => "not enough space on the heap",
            SPError::Param => "invalid parameter or parameter type",
            SPError::InvalidAddress => "invalid plugin address",
            SPError::NotFound => "object or index not found",
            SPError::Index => "invalid index or index not found",
            SPError::StackLow => "not enough space on the stack",
            SPError::NotDebugging => "debug section not found or debug not enabled",
            SPError::InvalidInstruction => "invalid instruction",
            SPError::MemAccess => "invalid memory access",
            SPError::StackMin => "stack went below stack boundary",
            SPError::HeapMin => "heap went below heap boundary",
            SPError::DivideByZero => "divide by zero",
            SPError::ArrayBounds => "array index is out of bounds",
            SPError::InstructionParam => "instruction contained invalid parameter",
            SPError::StackLeak => "stack memory leaked by native",
            SPError::HeapLeak => "heap memory leaked by native",
            SPError::ArrayTooBig => "dynamic array is too big",
            SPError::TrackerBounds => "tracker stack is out of bounds",
            SPError::InvalidNative => "native is not bound",
            SPError::ParamsMax => "maximum number of parameters reached",
            SPError::Native => "native detected error",
            SPError::NotRunnable => "plugin not runnable",
            SPError::Aborted => "call was aborted",
            SPError::CodeTooOld => "plugin format is too old",
            SPError::CodeTooNew => "plugin format is too new",
            SPError::OutOfMemory => "out of memory",
            SPError::IntegerOverflow => "integer overflow",
            SPError::Timeout => "script execution timed out",
            SPError::User => "custom error",
            SPError::Fatal => "fatal error",
        })
    }
}

impl Error for SPError {}

// ------------------------------------------------------------------------------------------------
// Identity / NativeInfo primitives
// ------------------------------------------------------------------------------------------------

pub struct IdentityToken {
    _private: [u8; 0],
}

pub type IdentityTokenPtr = *mut IdentityToken;

/// Struct to contain name/fnptr pairs for native registration.
///
/// SourceMod has very strict lifetime requirements for this data and you should not construct
/// instances of this type yourself - use the [`register_natives!`] macro instead.
#[repr(C)]
pub struct NativeInfo {
    pub name: *const c_char,
    pub func: Option<unsafe extern "C" fn(ctx: crate::IPluginContextPtr, args: *const cell_t) -> cell_t>,
}

// ------------------------------------------------------------------------------------------------
// Handle ID newtypes
// ------------------------------------------------------------------------------------------------

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct HandleTypeId(pub(crate) c_uint);

impl HandleTypeId {
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }

    pub fn invalid() -> Self {
        Self(0)
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct HandleId(pub(crate) c_uint);

impl HandleId {
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }

    pub fn invalid() -> Self {
        Self(0)
    }
}

impl From<cell_t> for HandleId {
    fn from(x: cell_t) -> Self {
        Self(x.0 as u32)
    }
}

impl From<HandleId> for cell_t {
    fn from(x: HandleId) -> Self {
        Self(x.0 as i32)
    }
}
