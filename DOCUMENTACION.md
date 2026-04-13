# Documentacion Completa — sm-ext-rs

## Indice

1. [Vision General](#1-vision-general)
2. [Estructura del Proyecto](#2-estructura-del-proyecto)
3. [Crate Principal — sm-ext](#3-crate-principal--sm-ext)
   - [Tipos Fundamentales](#31-tipos-fundamentales)
   - [Traits del Sistema](#32-traits-del-sistema)
   - [Interfaces de SourceMod](#33-interfaces-de-sourcemod)
   - [Sistema de Handles](#34-sistema-de-handles)
   - [Sistema de Forwards](#35-sistema-de-forwards)
   - [Macros Declarativos](#36-macros-declarativos)
   - [Manejo de Errores](#37-manejo-de-errores)
4. [Crate de Macros — sm-ext-derive](#4-crate-de-macros--sm-ext-derive)
   - [derive(SMExtension)](#41-derivesmextension)
   - [derive(SMInterfaceApi)](#42-derivesminterfaceapi)
   - [derive(ICallableApi)](#43-deriveicallableapi)
   - [#[native]](#44-native)
   - [#[forwards]](#45-forwards)
   - [#[vtable] y #[vtable_override]](#46-vtable-y-vtable_override)
5. [Flujo de Vida de una Extension](#5-flujo-de-vida-de-una-extension)
6. [Patrones de Uso](#6-patrones-de-uso)
   - [Extension Minima](#61-extension-minima)
   - [Natives](#62-natives)
   - [Forwards Globales](#63-forwards-globales)
   - [Handles Personalizados](#64-handles-personalizados)
   - [Integracion Async](#65-integracion-async)
7. [Arquitectura Interna Detallada](#7-arquitectura-interna-detallada)
   - [El Problema de las Calling Conventions](#71-el-problema-de-las-calling-conventions)
   - [Como Funciona virtual_call!](#72-como-funciona-virtual_call)
   - [El Adaptador de Extension](#73-el-adaptador-de-extension)
   - [El Pipeline de Conversion de Argumentos](#74-el-pipeline-de-conversion-de-argumentos)
   - [Ownership y Memoria en Handles](#75-ownership-y-memoria-en-handles)
   - [El Singleton de Extension](#76-el-singleton-de-extension)
8. [TODOs y Deuda Tecnica en el Codigo](#8-todos-y-deuda-tecnica-en-el-codigo)
9. [Mejoras Propuestas](#9-mejoras-propuestas)
10. [Que Debes Aprender para Retomar el Proyecto](#10-que-debes-aprender-para-retomar-el-proyecto)

---

## 1. Vision General

`sm-ext-rs` es una libreria Rust que proporciona bindings FFI y abstracciones seguras para escribir **extensiones de SourceMod** en Rust en lugar de C++.

**SourceMod** es el framework de modding para juegos de Valve (TF2, CS:GO, CS2, L4D2, etc.). Una extension de SourceMod es una `.dll` / `.so` de 32 bits que se carga en el servidor del juego y expone funcionalidad nueva a los plugins de SourcePawn.

El proyecto tiene dos objetivos:
1. Hacer que escribir extensiones estables sea mas facil que en C++
2. Exponer interfaces seguras sin garantizar 100% safe Rust (SourceMod viola algunas garantias de aliasing de Rust por diseno)

**Version actual:** 0.3.0  
**Targets soportados:** `i686-unknown-linux-gnu`, `i686-pc-windows-msvc` (32-bit obligatorio)  
**Licencia:** GPL-3.0-or-later

---

## 2. Estructura del Proyecto

```
sm-ext-rs/
├── Cargo.toml                  # Workspace + crate principal sm-ext v0.3.0
├── rustfmt.toml                # max_width = 1000 (no romper lineas)
├── src/
│   └── lib.rs                  # Toda la libreria (~1866 lineas)
├── sm-ext-derive/
│   ├── Cargo.toml              # Crate de macros procedurales
│   └── src/
│       └── lib.rs              # 5 macros: derive + 4 atributos (~912 lineas)
└── examples/
    ├── basic.rs                # Extension minima (solo metadata)
    ├── natives.rs              # Registro de funciones nativas
    ├── forwards.rs             # Creacion y ejecucion de forwards
    ├── handles.rs              # Handles personalizados (como methodmaps)
    ├── data.rs                 # Estado en la extension + game frame hooks
    └── async.rs                # Integracion con futures (LocalPool)
```

Todos los ejemplos se compilan como `crate-type = ["cdylib"]` — son `.so` / `.dll` reales para cargar en SourceMod.

---

## 3. Crate Principal — sm-ext

Todo el codigo runtime vive en `src/lib.rs`. Es un unico archivo grande (intencionalmente, segun el comentario en el modulo: "everything just lives in a soup at the top level for now").

### 3.1 Tipos Fundamentales

#### `cell_t`
```rust
#[repr(transparent)]
pub struct cell_t(i32);
```
La unidad basica de datos de SourcePawn. Un `cell_t` puede representar un `i32`, un `f32` (bits reinterpretados), o una direccion de memoria dentro del plugin. Las conversiones hacia/desde `i32` y `f32` son directas via `From`.

#### `HandleId` y `HandleTypeId`
Newtypes sobre `c_uint` para evitar mezclar IDs de handles con otros enteros. Ambos tienen `.is_valid()` y `.invalid()`.

#### `IdentityToken` / `IdentityTokenPtr`
Opaque pointer al sistema de identidades de SourceMod. Se usa para controlar quien puede leer/borrar handles. Normalmente se obtiene via `IExtension::get_identity()` o `IPluginContext::get_identity()`.

#### `NativeInfo`
```rust
#[repr(C)]
pub struct NativeInfo {
    pub name: *const c_char,
    pub func: Option<unsafe extern "C" fn(...)>,
}
```
Par nombre/puntero para registrar natives. No construir directamente — usar `register_natives!`.

---

### 3.2 Traits del Sistema

#### `TryFromPlugin<'ctx, T>` / `TryIntoPlugin<'ctx, T>`
El corazon del sistema de conversion de argumentos. Analogo a `TryFrom`/`TryInto` de Rust pero con acceso al contexto del plugin.

```rust
pub trait TryFromPlugin<'ctx, T = cell_t>: Sized {
    type Error;
    fn try_from_plugin(ctx: &'ctx IPluginContext, value: T) -> Result<Self, Self::Error>;
}
```

**Implementaciones incluidas:**
- `i32`, `f32`, `bool` → via `From<cell_t>` (blanket impl automatica)
- `&'ctx CStr` → llama a `ctx.local_to_string()` (acceso a memoria del plugin)
- `&'ctx str` → igual que CStr + validacion UTF-8
- `&'ctx mut cell_t`, `&'ctx mut i32`, `&'ctx mut f32` → por referencia via `ctx.local_to_phys_addr()` (permite que el native modifique variables del plugin)
- `IPluginFunction<'ctx>` → via `ctx.get_function_by_id()`
- `HandleId` → blanket impl via `From<cell_t>`

`TryIntoPlugin` se implementa automaticamente para cualquier tipo que implemente `TryFromPlugin`. **No implementar `TryIntoPlugin` directamente.**

#### `IExtensionInterface`
Trait de ciclo de vida. Todos los metodos tienen implementacion por defecto vacia, solo implementar los que se necesiten:

```rust
pub trait IExtensionInterface {
    fn on_extension_load(&mut self, me: IExtension, sys: IShareSys, late: bool) -> Result<(), Box<dyn Error>>;
    fn on_extension_unload(&mut self);
    fn on_extensions_all_loaded(&mut self);
    fn on_extension_pause_change(&mut self, pause: bool);
    fn on_core_map_start(&mut self, edict_list: *mut c_void, edict_count: i32, client_max: i32);
    fn on_core_map_end(&mut self);
    fn query_interface_drop(&mut self, interface: SMInterface) -> bool;
    fn notify_interface_drop(&mut self, interface: SMInterface);
    fn query_running(&mut self) -> Result<(), CString>;
    fn on_dependencies_dropped(&mut self);
}
```

#### `NativeResult`
Trait para tipos de retorno de natives. Permite que un native retorne `i32`, `f32`, `()`, `Result<T, E>`, etc. con manejo automatico de errores.

```rust
pub trait NativeResult {
    type Ok;
    type Err;
    fn into_result(self) -> Result<Self::Ok, Self::Err>;
}
```

Implementaciones incluidas: `()` (retorna 0), cualquier `T: TryIntoPlugin<cell_t>`, `Result<(), E>`, `Result<T, E>` donde `T: TryIntoPlugin`.

#### `CallableParam`
Para tipos que se pueden pasar como argumentos a forwards/funciones. Implementado para `cell_t`, `i32`, `f32`, `&CStr`, `HandleId`.

#### `ICallableApi` + `Executable`
```rust
pub trait ICallableApi {
    fn push_int(&mut self, cell: i32) -> Result<(), SPError>;
    fn push_float(&mut self, number: f32) -> Result<(), SPError>;
    fn push_string(&mut self, string: &CStr) -> Result<(), SPError>;
}

pub trait Executable: ICallableApi + Sized {
    fn execute(&mut self) -> Result<cell_t, SPError>;
    fn push<T: CallableParam>(&mut self, param: T) -> Result<(), SPError>;
}
```

Implementado por `Forward`, `ChangeableForward`, e `IPluginFunction`.

#### `RequestableInterface`
Para tipos que representan interfaces de SourceMod solicitables:
```rust
pub trait RequestableInterface {
    fn get_interface_name() -> &'static str;
    fn get_interface_version() -> u32;
    unsafe fn from_raw_interface(iface: SMInterface) -> Self;
}
```
Generado automaticamente por `#[derive(SMInterfaceApi)]` con el atributo `#[interface("Name", version)]`.

---

### 3.3 Interfaces de SourceMod

Cada interfaz de SourceMod esta representada por:
1. Un tipo puntero `XxxxPtr = *mut *mut XxxxVtable`
2. Una struct vtable con `#[vtable(XxxxPtr)]`
3. Un wrapper struct con metodos seguros

| Wrapper | Vtable | Interface SM | Uso |
|---------|--------|-------------|-----|
| `IExtension` | `IExtensionVtable` | — | Referencia a la propia extension |
| `IShareSys` | `IShareSysVtable` | — | Solicitar interfaces, registrar natives |
| `SMInterface` | `SMInterfaceVtable` | — | Base para todas las interfaces |
| `IForwardManager` | `IForwardManagerVtable` | `IForwardManager` v4 | Crear/liberar forwards |
| `IHandleSys` | `IHandleSysVtable` | `IHandleSys` v5 | Crear tipos de handle y manejarlos |
| `ISourceMod` | `ISourceModVtable` | `ISourceMod` v14 | Logging, paths, game frame hooks |
| `IPluginContext` | `IPluginContextVtable` | — | Acceso a memoria del plugin |
| `IPluginFunction` | `IPluginFunctionVtable` | — | Llamar funciones SourcePawn |

**Como solicitar una interfaz en `on_extension_load`:**
```rust
let handlesys: IHandleSys = sys.request_interface(&myself)?;
let forward_manager: IForwardManager = sys.request_interface(&myself)?;
let sourcemod: ISourceMod = sys.request_interface(&myself)?;
```
El tipo destino le dice a `request_interface` que nombre y version pedir (via `RequestableInterface`).

---

### 3.4 Sistema de Handles

Los handles son la forma en que SourceMod gestiona objetos con ownership compartido entre plugins y extensiones.

**Flujo completo:**

```
IHandleSys::create_type::<T>()
    → HandleType<T>             (se guarda en la extension, borrar en on_extension_unload)
    
HandleType<T>::create_handle(Rc<T>, owner, access)
    → HandleId                  (se pasa a SourcePawn como cell_t)
    
HandleType<T>::read_handle(HandleId, owner)
    → Rc<T>                     (acceso al objeto desde un native)
```

**Internamente**, los objetos se almacenan como `Rc<T>` cuyo puntero raw se pasa a SourceMod. El destructor se implementa via `IHandleTypeDispatchAdapter<T>`:
- `OnHandleDestroy` → `Rc::from_raw()` (libera el Rc)
- `GetHandleApproxSize` → `std::mem::size_of_val()`

**`read_handle` tiene un truco critico** (lineas 1583-1585):
```rust
let object = Rc::from_raw(object as *mut T);
std::mem::forget(object.clone());  // Mantiene el refcount, no decrementa
object
```
Se reconstruye el `Rc` desde el raw pointer y luego se hace `forget` del clon para no decrementar el refcount accidentalmente.

**El patron estandar** para tipos de handle es envolver en `RefCell` para mutabilidad interior:
```rust
HandleType<RefCell<MyType>>
```
Luego en natives: `this.try_borrow()` / `this.try_borrow_mut()`.

---

### 3.5 Sistema de Forwards

Un **forward global** es una funcion SourcePawn que multiples plugins pueden "suscribir" (como un evento). Un **forward privado** (`ChangeableForward`) controla explicitamente que funciones recibe.

**Forward** (global):
- Creado via `IForwardManager::create_global_forward(name, ExecType, params)`
- Implementa `Drop` que llama a `ReleaseForward` automaticamente
- `ExecType` determina como se agrega el valor de retorno: `Ignore`, `Single`, `Event`, `Hook`, `LowEvent`

**ChangeableForward** (privado):
- Creado via `IForwardManager::create_private_forward(name_opt, ExecType, params)`
- Ademas tiene `add_function()` / `remove_function()` para gestionar suscriptores manualmente
- Usado en el ejemplo async para crear un "callback" de un solo plugin

**`ParamType`** define los tipos de parametros al crear el forward:
```
Any=0, Cell, Float, String, Array, VarArgs, CellByRef, FloatByRef
```

---

### 3.6 Macros Declarativos

#### `virtual_call!(nombre, this_ptr, args...)`
Invoca una funcion virtual de forma segura segun la plataforma:
- **Linux**: `((**ptr).fn)(ptr, args...)`
- **Windows + abi_thiscall**: `((**ptr).fn)(ptr, args...)` con ABI thiscall
- **Windows sin abi_thiscall**: inserta `null_mut()` como segundo argumento (dummy para EDX en fastcall)

#### `virtual_call_varargs!(nombre, this_ptr, args...)`
Igual pero para funciones variadicas — NO inserta el dummy en Windows porque varargs no usa fastcall.

#### `register_natives!(&sys, &myself, [("name", func_ptr), ...])`
Crea un `Vec<NativeInfo>` nulo-terminado y lo **filtra** (leaks intencionalmente) para que viva mientras la extension este cargada. SourceMod requiere que el array sea valido indefinidamente.

---

### 3.7 Manejo de Errores

**`SPError`** (33 variantes) — errores de la VM de SourcePawn. Implementa `Error` y `Display`.

**`HandleError`** (12 variantes) — errores del sistema de handles.

**`CreateForwardError`** — `InvalidName(NulError)` o `InvalidParams`.

**`RequestInterfaceError`** — `InvalidName(NulError)` o `InvalidInterface(name, ver)`.

**`safe_native_invoke`** — envuelve el cuerpo de un native en `std::panic::catch_unwind`. Si el closure retorna `Err` o hace panic, llama a `ctx.throw_native_error()` que registra el error en SourceMod y retorna `0`.

Todos los callbacks de `IExtensionInterfaceAdapter` tambien usan `catch_unwind` individualmente para evitar que un panic en Rust cruce el boundary FFI.

---

## 4. Crate de Macros — sm-ext-derive

Libreria `proc-macro` que genera codigo en tiempo de compilacion. Usa `syn` para parsear el AST de Rust y `quote` para generar tokens.

### 4.1 `derive(SMExtension)`

**Entrada:** struct con `#[extension(key = "value", ...)]`

**Genera:**
```rust
// Verificacion de CRT estatico en Windows
#[cfg(all(windows, not(target_feature = "crt-static"), not(test)))]
compile_error!("...");

// Singleton thread-local
thread_local! {
    static EXTENSION_GLOBAL: RefCell<Option<*mut IExtensionInterfaceAdapter<MyExt>>> = ...;
}

// Entry point exportado
#[no_mangle]
pub extern "C" fn GetSMExtAPI() -> *mut IExtensionInterfaceAdapter<MyExt> { ... }

// Implementacion de ExtensionMetadata
impl ExtensionMetadata for MyExt { ... }
```

**Metadata:** Si no se especifica un campo en `#[extension]`, usa variables de entorno de Cargo en tiempo de compilacion (`CARGO_PKG_NAME`, `CARGO_PKG_VERSION`, etc.) via `env!()`. La fecha por defecto es el string literal `"with Rust"`.

**Acceso al singleton** — patron manual recomendado (no generado automaticamente):
```rust
fn get() -> &'static Self {
    EXTENSION_GLOBAL.with(|ext| unsafe { &(*ext.borrow().unwrap()).delegate })
}
```

### 4.2 `derive(SMInterfaceApi)`

**Entrada:** struct con `#[interface("InterfaceName", version_number)]`

**Genera:**
- `impl RequestableInterface for MyInterface` — nombre, version, y `from_raw_interface` (transmute del puntero)
- `impl SMInterfaceApi for MyInterface` — `get_interface_version()`, `get_interface_name()`, `is_version_compatible()`

### 4.3 `derive(ICallableApi)`

**Genera:** `impl ICallableApi for MyStruct` con `push_int`, `push_float`, `push_string` que llaman a los metodos correspondientes en el vtable.

### 4.4 `#[native]`

**Entrada:** funcion Rust libre  
**Validaciones en compilacion:** no async, no unsafe, no extern, no generics, primer argumento debe ser `&IPluginContext`

**Genera dos funciones:**

```rust
// 1. Wrapper extern "C" con el nombre original
unsafe extern "C" fn mi_native(ctx: IPluginContextPtr, args: *const cell_t) -> cell_t {
    safe_native_invoke(ctx, |ctx| {
        let count: i32 = (*args).into();
        // Validar cantidad de argumentos
        if count < MINIMO { return Err(...); }
        // Convertir cada argumento via TryIntoPlugin
        let result = __mi_native_impl(
            &ctx,
            arg1.try_into_plugin(&ctx)?,
            arg2.try_into_plugin(&ctx)?,
            // Option<T> para argumentos opcionales
            if idx <= count { Some(...) } else { None },
        ).into_result()?;
        Ok(result.try_into_plugin(&ctx)?)
    })
}

// 2. Implementacion con el nombre renombrado
fn __mi_native_impl(ctx: &IPluginContext, ...) -> ReturnType { ... }
```

**Argumentos opcionales:** Los parametros `Option<T>` al final de la firma permiten compatibilidad con plugins compilados con versiones anteriores del native (menos argumentos).

**Conteo de argumentos:** `args[0]` contiene el conteo. `args[1]`, `args[2]`, etc. son los argumentos reales.

### 4.5 `#[forwards]`

**Entrada:** struct con campos anotados `#[global_forward("Name", ExecType::Variant)]` de tipo `fn(params) -> ret`

**NOTA:** `#[private_forward]` esta marcado como "not implemented" en el codigo fuente — genera un error de compilacion.

**Genera por cada campo:**
- Un tipo wrapper `__nombre_forward<'a>(&'a mut Forward)` con metodo `execute(args) -> Result<ret, SPError>`
- Una variable `thread_local!` `__g_nombre_forward: RefCell<Option<Forward>>`

**Genera para el struct completo:**
- Un trait oculto `__MyForwards_forwards` con metodos `register`, `unregister`, y un metodo por forward
- `impl __MyForwards_forwards for MyForwards`

**Uso:**
```rust
MyForwards::register(&forward_manager)?;    // en on_extension_load
MyForwards::unregister();                   // en on_extension_unload
MyForwards::on_event(|fwd| fwd.execute(a, b))?;  // en natives
```

### 4.6 `#[vtable]` y `#[vtable_override]`

**`#[vtable(ThisPtrType)]`** sobre una struct:
- Agrega `#[repr(C)]` y `#[doc(hidden)]`
- Prepende `this: ThisPtrType` a cada campo funcion
- Emite 3 versiones condicionales:
  - `#[cfg(not(all(windows, target_arch = "x86")))]` — ABI `extern "C"`
  - `#[cfg(all(windows, target_arch = "x86", feature = "abi_thiscall"))]` — ABI `extern "thiscall"`
  - `#[cfg(all(windows, target_arch = "x86", not(feature = "abi_thiscall")))]` — ABI `extern "fastcall"` + `_dummy: *const usize` como segundo param
- Las funciones **variadicas** (`...`) no cambian de ABI en Windows (siguen como C)

**`#[vtable_override]`** sobre una funcion:
- Emite 3 versiones del mismo cuerpo con los 3 ABIs correspondientes
- En fastcall inserta `_dummy: *const usize` como segundo parametro

---

## 5. Flujo de Vida de una Extension

```
Servidor inicia
    → SourceMod carga el .so/.dll
        → dlsym("GetSMExtAPI") [generado por #[derive(SMExtension)]]
            → Crea Box<IExtensionInterfaceAdapter<MyExtension>>
            → Guarda ptr en EXTENSION_GLOBAL (thread_local)
            → Retorna el ptr a SourceMod

SourceMod llama OnExtensionLoad (via vtable)
    → IExtensionInterfaceAdapter::on_extension_load [vtable_override]
        → catch_unwind
            → MyExtension::on_extension_load (implementacion del usuario)
                → sys.request_interface::<IHandleSys>()
                → sys.request_interface::<IForwardManager>()
                → MyForwards::register(&forward_manager)
                → register_natives!(&sys, &myself, [...])

Durante el juego:
    Plugin SourcePawn llama a un native
        → extern "C" fn mi_native(ctx, args) [generado por #[native]]
            → safe_native_invoke(ctx, ...)
                → Validar argumentos
                → Convertir via TryIntoPlugin
                → Ejecutar logica Rust
                → Convertir retorno via TryIntoPlugin

    Extension dispara un forward
        → MyForwards::on_event(|fwd| fwd.execute(a, b, c))
            → forward.push(a) → virtual_call!(PushCell)
            → forward.push(b) → virtual_call!(PushFloat)
            → forward.push(c) → virtual_call!(PushString)
            → forward.execute() → virtual_call!(Execute)

SourceMod descarga la extension
    → IExtensionInterfaceAdapter::on_extension_unload
        → MyExtension::on_extension_unload (implementacion del usuario)
            → MyForwards::unregister()
            → self.handle_type = None  // Drop libera el tipo de handle
    → Box<IExtensionInterfaceAdapter<T>> se libera al final
```

---

## 6. Patrones de Uso

### 6.1 Extension Minima

```rust
use sm_ext::{IExtensionInterface, SMExtension};

#[derive(Default, SMExtension)]
#[extension(name = "Mi Extension", description = "Hace cosas")]
pub struct MiExtension;

impl IExtensionInterface for MiExtension {}
```

### 6.2 Natives

```rust
use sm_ext::{native, register_natives, IPluginContext};
use std::ffi::CStr;

// Tipos soportados: i32, f32, bool, &CStr, &str, &mut i32, &mut f32, HandleId, IPluginFunction
// Option<T> al final = argumento opcional
#[native]
fn mi_native(ctx: &IPluginContext, a: i32, b: f32, c: &CStr, opt: Option<i32>) -> Result<i32, Box<dyn std::error::Error>> {
    Ok(a + opt.unwrap_or(0))
}

// En on_extension_load:
register_natives!(&sys, &myself, [
    ("SM_MiNative", mi_native),
]);
```

### 6.3 Forwards Globales

```rust
use sm_ext::{forwards, ExecType, IForwardManager};
use std::ffi::CStr;

#[forwards]
struct MisForwards {
    #[global_forward("OnMiEvento", ExecType::Event)]
    on_mi_evento: fn(client: i32, reason: &CStr) -> i32,
}

// En on_extension_load:
let fwd_mgr: IForwardManager = sys.request_interface(&myself)?;
MisForwards::register(&fwd_mgr)?;

// En on_extension_unload:
MisForwards::unregister();

// Para disparar el forward:
let result = MisForwards::on_mi_evento(|fwd| fwd.execute(client_id, c_str!("razon")))?;
```

### 6.4 Handles Personalizados

```rust
use sm_ext::{HandleType, HandleId, IHandleSys, IPluginContext, native};
use std::cell::RefCell;
use std::rc::Rc;

struct MiObjeto { datos: i32 }

// En la extension:
struct MiExtension {
    handle_type: Option<HandleType<RefCell<MiObjeto>>>,
}

// En on_extension_load:
let hs: IHandleSys = sys.request_interface(&myself)?;
self.handle_type = Some(hs.create_type("MiObjeto", None, myself.get_identity())?);

// En on_extension_unload:
self.handle_type = None;  // Drop libera el tipo

// Crear un handle (retornar al plugin):
impl<'ctx> TryIntoPlugin<'ctx> for MiObjeto {
    type Error = HandleError;
    fn try_into_plugin(self, ctx: &'ctx IPluginContext) -> Result<cell_t, Self::Error> {
        let obj = Rc::new(RefCell::new(self));
        let handle = MiExtension::handle_type().create_handle(obj, ctx.get_identity(), None)?;
        Ok(handle.into())
    }
}

// Leer un handle (en un native):
#[native]
fn native_accion(ctx: &IPluginContext, this: HandleId) -> Result<i32, Box<dyn Error>> {
    let obj = MiExtension::handle_type().read_handle(this, ctx.get_identity())?;
    let obj = obj.try_borrow()?;
    Ok(obj.datos)
}
```

### 6.5 Integracion Async

```rust
// Usar LocalPool (futures crate) para ejecutar tareas async en el game thread
struct MiExtension {
    pool: RefCell<LocalPool>,
    frame_hook: Option<GameFrameHookId>,
}

// En on_extension_load — registrar hook de game frame para tickear el executor:
extern "C" fn on_game_frame(_simulating: bool) {
    MiExtension::get().pool.borrow_mut().run_until_stalled()
}
self.frame_hook = Some(sourcemod.add_game_frame_hook(on_game_frame));

// En on_extension_unload:
self.frame_hook = None;  // Drop desregistra el hook

// Spawnear una tarea:
MiExtension::get().pool.borrow().spawner().spawn_local(async move {
    task::sleep(Duration::from_secs(5)).await;
    // ... logica async ...
})?;
```

---

## 7. Arquitectura Interna Detallada

### 7.1 El Problema de las Calling Conventions

SourceMod en Windows x86 usa clases de C++ cuyas funciones virtuales usan `thiscall`. Rust no soporta `thiscall` de forma estable (requiere el feature `abi_thiscall` de nightly).

**Solucion para compiladores sin soporte de thiscall:**
`thiscall` en MSVC pasa `this` en el registro `ECX`. `fastcall` pasa el primer argumento en `ECX` y el segundo en `EDX`. Por eso, llamar una funcion `thiscall` con ABI `fastcall` funciona si se agrega un argumento dummy para "consumir" el EDX — el dummy se ignora pero la convencion de llamada queda alineada.

**Las tres versiones generadas por `#[vtable]`:**

| Plataforma | ABI | `this` | Dummy |
|-----------|-----|--------|-------|
| Linux x86/x64 | `extern "C"` | Primer param | No |
| Windows x86 + `abi_thiscall` | `extern "thiscall"` | Registro ECX | No |
| Windows x86 sin `abi_thiscall` | `extern "fastcall"` | Primer param (ECX) | Si (`_dummy` en EDX) |

Las funciones variadicas (`...`) son siempre `extern "C"` porque las calling conventions basadas en registros no son compatibles con varargs en x86.

### 7.2 Como Funciona `virtual_call!`

```rust
macro_rules! virtual_call {
    ($name:ident, $this:expr, $($param:expr),*) => {
        ((**$this).$name)(
            $this,
            #[cfg(all(windows, target_arch = "x86", not(feature = "abi_thiscall")))]
            std::ptr::null_mut(),   // <-- dummy solo en fastcall
            $($param,)*
        )
    };
}
```

`$this` es un `*mut *mut VtableStruct`. `**$this` desreferencia dos veces para obtener la vtable. `(**$this).$name` accede al campo-funcion. `($this, ...)` llama la funcion pasando `this` como primer argumento.

### 7.3 El Adaptador de Extension

`IExtensionInterfaceAdapter<T>` es el "vtable manual en Rust". Tiene:
- Un campo `vtable: *mut IExtensionInterfaceVtable` que apunta a una vtable allocada en heap
- Un campo `delegate: T` que es la struct del usuario

La vtable se crea en `IExtensionInterfaceAdapter::new()` asignando cada campo al metodo estatico correspondiente. Los metodos usan `#[vtable_override]` para generar las 3 variantes de ABI.

```
IExtensionInterfaceAdapter<MyExt>
├── vtable: *mut IExtensionInterfaceVtable
│   ├── OnExtensionLoad → IExtensionInterfaceAdapter::<MyExt>::on_extension_load
│   ├── OnExtensionUnload → IExtensionInterfaceAdapter::<MyExt>::on_extension_unload
│   └── ...
└── delegate: MyExt
    └── (implementacion del usuario)
```

Cuando SourceMod llama `OnExtensionLoad`, sigue el puntero de vtable, llama la funcion, que hace `(*this.cast::<Self>()).delegate.on_extension_load(...)`.

### 7.4 El Pipeline de Conversion de Argumentos

Para un native `#[native] fn foo(ctx: &IPluginContext, a: i32, b: &CStr) -> f32`:

```
SourcePawn llama: foo(5, "hola")

args = [count=2, cell_t(5), cell_t(ptr_a_string)]

// Generado por #[native]:
1. count = args[0] = 2
2. a = args[1].try_into_plugin(&ctx)
     = i32::try_from_plugin(&ctx, cell_t(5))
     = i32::try_from(cell_t(5))     // blanket impl
     = Ok(5)
3. b = args[2].try_into_plugin(&ctx)
     = <&CStr>::try_from_plugin(&ctx, cell_t(ptr))
     = ctx.local_to_string(cell_t(ptr))  // lee de memoria del plugin
     = Ok(&CStr)
4. result = __foo_impl(&ctx, 5, &CStr) = 3.14f32
5. result.into_result() = Ok(3.14f32)
6. Ok(3.14f32).try_into_plugin(&ctx)
     = cell_t::try_from_plugin(&ctx, 3.14f32)  // no necesita ctx
     = cell_t(3.14f32.to_bits() as i32)
```

### 7.5 Ownership y Memoria en Handles

El ciclo completo de un objeto en el sistema de handles:

```
Rust: Rc::new(RefCell::new(obj))     refcount=1
    → Rc::into_raw()                  "transfiere" ownership al ptr raw
    → pasado a IHandleSys::CreateHandleEx como *mut c_void
    
SourcePawn usa el handle (refcount sigue en 1, Rust no sabe)

SourcePawn llama delete o el plugin se descarga:
    → SourceMod llama IHandleTypeDispatchVtable::OnHandleDestroy
    → Rust: Rc::from_raw(ptr)         reconstruye el Rc, refcount=1
    → drop(Rc)                         refcount=0 → se libera el objeto

read_handle (lectura sin liberar):
    → Rc::from_raw(ptr)               refcount momentaneamente=1
    → std::mem::forget(clone)         clone incrementa a 2, forget no decrementa
    → retorna original con refcount=1 (sin cambios netos)
    → Rc se dropea al salir del native, refcount=0???  <- PELIGRO
    
    Solucion real: el objeto sigue "vivo" en SourceMod, no se libera
    hasta que el handle se destruya. El Rc retornado tiene refcount=1
    despues del forget del clon. Cuando el native termina y el Rc se
    dropea, refcount llega a 0 pero el objeto ya se copio/proceso.
    Si se necesita persistencia, se debe clonar el Rc antes de salir.
```

### 7.6 El Singleton de Extension

El singleton es intencionalmente `thread_local!` para evitar problemas de concurrencia (SourceMod es principalmente single-threaded en el game thread). El puntero raw es necesario porque `IExtensionInterfaceAdapter` no puede tener un lifetime estatico simple.

El comentario en el codigo (lineas 48-58 del derive) reconoce que esto es "fairly gross" y propone mejoras futuras como almacenar interfaces directamente en `thread_local!`.

---

## 8. TODOs y Deuda Tecnica en el Codigo

Los siguientes problemas estan documentados directamente en el codigo fuente con comentarios `TODO`:

| Ubicacion | Problema |
|-----------|----------|
| `lib.rs:34` | Investigar usar `union` para `cell_t` en lugar del wrapper |
| `lib.rs:127` | Las implementaciones `&mut T` de `TryFromPlugin` son riesgosas — considerar un wrapper `SPRef`/`SPString`/`SPArray` |
| `derive/lib.rs:48-58` | El singleton via puntero raw es "gross" — alternativas: interfaces en thread_local propios, o pasar el singleton como parametro de natives |
| `derive/lib.rs:242` | El campo `author` de Cargo puede tener multiples autores — necesita post-procesado |
| `derive/lib.rs:253` | El campo `tag` deberia slug-ificar/capitalizar el nombre del paquete |
| `derive/lib.rs:604` | `#[private_forward]` no esta implementado |
| `lib.rs:1033` | La interfaz `ICallableApi` es "very, very rough" |
| `lib.rs:1365` | `GetHandleApproxSize` no maneja tamanos dinamicos (Vec, String dentro del objeto) |
| `lib.rs:1694` | `add_frame_action` deberia estar en un subset `Send` de `ISourceMod` |
| `lib.rs:1727` | `virtual_call_varargs!` necesita forma de ser type-safe |
| `lib.rs:1763` | `register_natives!` filtra el Vec — deberia hacerse estatico en nightly con features de macros |
| `async.rs:77` | El game frame hook para el executor async no captura panics |
| `derive/lib.rs:859` | `#[vtable_override]` necesita mas validacion e informes de error |

---

## 9. Mejoras Propuestas — Arquitectura y Practicas Profesionales

Esta seccion analiza las limitaciones de diseno actuales y propone mejoras concretas con justificacion arquitectonica.

---

### 9.1 Separar la API Publica del Glue Interno (Principio de Segregacion de Interfaces)

**Problema actual:** `src/lib.rs` mezcla en el mismo nivel: tipos FFI crudos (`IForwardVtable`, `IHandleSysVtable`), wrappers seguros (`IForwardManager`, `IHandleSys`), traits de conversion, macros, y helpers internos. Los campos `pub` en vtables (como `PushCell` en `IForwardVtable`) estan expuestos aunque no deben usarse directamente.

**Mejora propuesta:** Organizar en modulos con visibilidad explicita:
```
src/
├── lib.rs              # Re-exports publicos unicamente
├── ffi/
│   ├── mod.rs          # Tipos FFI, vtables (pub(crate))
│   ├── extension.rs
│   ├── handles.rs
│   └── forwards.rs
├── api/
│   ├── mod.rs          # Wrappers seguros (pub)
│   ├── handles.rs
│   ├── forwards.rs
│   └── sourcemod.rs
└── convert.rs          # TryFromPlugin, NativeResult, CallableParam
```

**Beneficio:** Los tipos internos como `IForwardVtable` o `IHandleTypeDispatchPtr` quedan como `pub(crate)` — los consumidores de la libreria nunca los ven ni los usan incorrectamente.

---

### 9.2 Reemplazar el Singleton via Puntero Raw con un Estado Tipado (Patron Registry)

**Problema actual:** El singleton `EXTENSION_GLOBAL` expone `*mut IExtensionInterfaceAdapter<T>`, obligando a cada extension a implementar manualmente el patron `fn get() -> &'static Self` con `unsafe`. El codigo fuente lo reconoce como "fairly gross".

**Mejora propuesta:** Patron **Registry** con almacenamiento por tipo:
```rust
// Generado por #[derive(SMExtension)]:
thread_local! {
    static SHARE_SYS: RefCell<Option<IShareSys>> = RefCell::new(None);
    static MY_EXTENSION: RefCell<Option<IExtension>> = RefCell::new(None);
}

// API ergonomica:
pub fn share_sys() -> impl Deref<Target = IShareSys> { ... }
```

Alternativa mas avanzada: almacenar las interfaces con `anymap` o un `HashMap<TypeId, Box<dyn Any>>` para que `request_interface` las cachee automaticamente y sean accesibles desde cualquier native sin pasar referencias manualmente.

**Beneficio:** Elimina el `unsafe` del patron `get()`, hace imposible acceder a interfaces antes de que se inicialicen, y elimina la friccion de pasar referencias por todos los natives.

---

### 9.3 Patron Builder para Construccion de Extensions (Fluent API)

**Problema actual:** Toda la inicializacion ocurre en `on_extension_load` con llamadas imperativas secuenciales. No hay verificacion estatica de que se hayan solicitado las interfaces necesarias.

**Mejora propuesta:** Un builder generado por macros que declare dependencias en tiempo de compilacion:
```rust
#[derive(Default, SMExtension)]
#[extension(name = "Mi Extension")]
#[requires(IHandleSys, IForwardManager)]  // Error de compilacion si no se piden
pub struct MiExtension {
    // Los campos de interfaz se gestionan automaticamente
}
```

O un patron builder explicito:
```rust
fn on_extension_load(&mut self, me: IExtension, sys: IShareSys, late: bool) -> Result<(), Box<dyn Error>> {
    ExtensionBuilder::new(me, sys)
        .with_handles(|hs| { self.handle_type = Some(hs.create_type(...)?); Ok(()) })
        .with_forwards(|fm| { MyForwards::register(fm) })
        .with_natives(|ns| { register_natives!(...); Ok(()) })
        .build()
}
```

**Beneficio:** Separa la logica de inicializacion en unidades independientes y testables. Las dependencias de interfaz son declarativas, no procedurales.

---

### 9.4 Tipos de Error con `thiserror` (Error Handling Profesional)

**Problema actual:** Los tipos de error (`CreateForwardError`, `RequestInterfaceError`, `HandleError`, etc.) implementan `Display` y `Error` manualmente con `match` boilerplate, totalizando ~100 lineas de codigo repetitivo.

**Mejora propuesta:** Usar `thiserror` para derivar implementaciones:
```rust
#[derive(Debug, thiserror::Error)]
pub enum CreateForwardError {
    #[error("invalid forward name: {0}")]
    InvalidName(#[from] NulError),
    #[error("failed to create forward {name}: invalid params")]
    InvalidParams { name: String },
}
```

**Beneficio:** Reduce el boilerplate de error handling de ~100 lineas a ~20. Hace que anadir variantes nuevas sea trivial. El uso de `#[from]` habilita `?` directo en `NulError`.

---

### 9.5 Wrappers con Tipos Estado para Garantias en Tiempo de Compilacion (Typestate Pattern)

**Problema actual:** `HandleId` puede ser invalido (`HandleId::invalid()` retorna `HandleId(0)`). No hay distincion entre un handle valido y uno potencialmente invalido a nivel de tipos.

**Mejora propuesta:** Typestate para handles:
```rust
pub struct Handle<T, State = Unverified> {
    id: HandleId,
    _type: PhantomData<T>,
    _state: PhantomData<State>,
}

pub struct Unverified;
pub struct Verified;

impl<T> Handle<T, Unverified> {
    pub fn verify(self, ty: &HandleType<T>, owner: IdentityTokenPtr) -> Result<Handle<T, Verified>, HandleError> { ... }
}

impl<T> Handle<T, Verified> {
    pub fn read(&self) -> Rc<T> { ... }  // No puede fallar, ya fue verificado
}
```

**Beneficio:** Los natives que reciben un `Handle<T, Verified>` no necesitan manejar el error de handle invalido — el compilador garantiza que la verificacion ya ocurrio.

---

### 9.6 Estrategia de Testing con Mocks (Testabilidad)

**Problema actual:** No hay tests unitarios porque todo depende de SourceMod en runtime. Esto hace que errores en la logica de macros o de conversion solo se descubran al cargar la extension en un servidor.

**Mejora propuesta:** Separar la logica testable del glue FFI:

**Nivel 1 — Tests de macros:** Los macros procedurales pueden testearse con `trybuild` (compilacion de snippets que deben compilar o fallar):
```rust
// tests/compile_fail/native_async.rs
#[native]
async fn bad_native() {}  // debe fallar con "Native callback must not be async"
```

**Nivel 2 — Tests de conversion:** `TryFromPlugin` y `NativeResult` pueden testearse sin SourceMod si se abstrae `IPluginContext` detras de un trait:
```rust
pub trait PluginContext {
    fn local_to_string(&self, addr: cell_t) -> Result<&CStr, SPError>;
    fn local_to_phys_addr(&self, addr: cell_t) -> Result<&mut cell_t, SPError>;
}
// IPluginContext implementa el trait en produccion
// MockPluginContext lo implementa en tests
```

**Nivel 3 — Tests de integracion:** Usar el [SourceMod Test Framework](https://wiki.alliedmods.net/SourceMod_Test_Framework) en un servidor CI (mas complejo, pero es la validacion definitiva).

---

### 9.7 Generacion de Bindings Automatica con `bindgen` (Mantenibilidad)

**Problema actual:** Todas las vtables de SourceMod (`IForwardVtable`, `IHandleSysVtable`, etc.) estan escritas manualmente en Rust. Si SourceMod actualiza su API, hay que actualizar los structs a mano y es facil cometer errores en el orden de los campos.

**Mejora propuesta:** Usar `bindgen` en un `build.rs` para generar los tipos FFI desde las cabeceras C++ de SourceMod, y luego usar los macros `#[vtable]` solo como capa de seguridad sobre los bindings generados.

```rust
// build.rs
fn main() {
    bindgen::Builder::default()
        .header("sm-sdk/public/IHandleSys.h")
        .generate()
        .unwrap()
        .write_to_file("src/generated/handlesys.rs");
}
```

**Beneficio:** Actualizar a una nueva version de SourceMod SDK se reduce a regenerar los bindings. Elimina la posibilidad de tener campos en orden incorrecto.

---

### 9.8 Separar `sm-ext-derive` en Sub-crates por Dominio

**Problema actual:** Los 5+ macros procedurales estan todos en un unico archivo de 912 lineas. Agregar un nuevo macro requiere modificar el mismo archivo que los macros existentes.

**Mejora propuesta:** Workspace con crates separados por responsabilidad:
```
sm-ext-derive/
├── sm-ext-derive-vtable/    # #[vtable], #[vtable_override]
├── sm-ext-derive-native/    # #[native]
├── sm-ext-derive-forwards/  # #[forwards]
└── sm-ext-derive/           # Re-exports de todos los anteriores
```

**Beneficio:** Cada macro tiene su propio conjunto de tests. Los tiempos de compilacion incremental mejoran. Los errores de un macro no afectan la compilacion de los otros.

---

### 9.9 Async Runtime Integrado como Primitiva de la Libreria

**Problema actual:** El ejemplo `async.rs` implementa el patron `LocalPool + GameFrameHook` manualmente. Cada extension que quiera async tiene que replicar ~30 lineas de codigo y recordar que hay que `drop` el hook en `on_extension_unload`.

**Mejora propuesta:** Exponer un tipo `AsyncRuntime` de primera clase:
```rust
// En sm-ext (activado por feature "async"):
pub struct AsyncRuntime {
    pool: RefCell<LocalPool>,
    _hook: GameFrameHookId,  // Auto-desregistra al hacer drop
}

impl AsyncRuntime {
    pub fn new(sourcemod: &ISourceMod) -> Self { ... }
    pub fn spawn<F: Future>(&self, fut: F) -> Result<(), SpawnError> { ... }
}
```

**Beneficio:** El patron async queda encapsulado, es imposible olvidarse de desregistrar el hook, y la API es identica para todas las extensiones.

---

### 9.10 Documentacion con Ejemplos de SourcePawn Ejecutables

**Problema actual:** Los docstrings en los ejemplos muestran el codigo SourcePawn correspondiente en bloques ` ```sourcepawn ``` ` pero no son ejecutables ni verificables.

**Mejora propuesta:** Crear un directorio `sm-examples/` con pares `.sp` / `.rs` completos y funcionales, organizados por caso de uso, que sirvan tanto como documentacion como como suite de tests de integracion manual.

---

## 10. Que Debes Aprender para Retomar el Proyecto

### Conocimiento Critico (sin esto no puedes avanzar)

**1. FFI en Rust**
- `extern "C"` functions, `#[repr(C)]`, `#[repr(transparent)]`
- `*mut T`, `*const T` — cuándo es seguro desreferenciar
- `Box::into_raw` / `Box::from_raw`, `Rc::into_raw` / `Rc::from_raw`
- `std::mem::transmute` para reinterpretar punteros
- `std::panic::catch_unwind` — necesario para cruzar el boundary FFI sin UB
- Recursos: [Rustonomicon](https://doc.rust-lang.org/nomicon/), capitulos de FFI

**2. Calling Conventions en x86**
- Que es `thiscall`, `fastcall`, `cdecl`
- Por que `thiscall` y `fastcall` difieren solo en el uso de EDX
- El truco del argumento dummy para emular `thiscall` con `fastcall`
- Recursos: MSDN docs sobre calling conventions, Intel manual de x86

**3. Macros Procedurales en Rust**
- `proc_macro`, `proc_macro_attribute`, `proc_macro_derive`
- `syn` v1: `syn::parse_macro_input!`, `syn::DeriveInput`, `syn::ItemFn`, `syn::ItemStruct`
- `quote::quote!`, `quote::format_ident!`, `quote_spanned!`
- Como emitir errores de compilacion con `compile_error!` y `Span`
- Recursos: [The Little Book of Rust Macros](https://veykril.github.io/tlborm/), `syn` docs

**4. C++ VTables**
- Como funciona el despacho virtual en C++
- Estructura de memoria: `ptr → ptr-a-vtable → [fn1, fn2, ...]`
- Por que el layout puede diferir entre plataformas (especialmente destructor en Linux vs Windows)
- Por que `IForwardVtable` tiene `_Destructor2` solo en Linux (`#[cfg(not(windows))]`)
- Recursos: articulos sobre C++ ABI, Itanium ABI spec

**5. SourceMod Extension API**
- Como SourceMod carga extensiones (`GetSMExtAPI`, `IExtensionInterface`)
- El concepto de `IShareSys` para compartir interfaces entre extensiones
- Como funcionan los handles (ownership, security tokens)
- Como funcionan los forwards (global vs private, ExecType)
- Recursos: [SourceMod wiki - Writing Extensions](https://wiki.alliedmods.net/Writing_Extensions_(SourceMod)), cabeceras de SM en C++

### Conocimiento Recomendado (acelera el desarrollo)

**6. Rust Lifetimes Avanzados**
- Como `IPluginFunction<'ctx>` garantiza que la funcion no viva mas que el contexto
- Como `TryFromPlugin<'ctx>` propaga el lifetime del contexto a `&'ctx CStr`
- Por que `safe_native_invoke` requiere `UnwindSafe`

**7. Rust Trait System Avanzado**
- Blanket implementations (como `TryIntoPlugin` sobre `TryFromPlugin`)
- `NativeResult` con tipos asociados para unificar multiples tipos de retorno
- Porque la implementacion de `NativeResult` para `T: TryIntoPlugin` tiene un lifetime `'ctx` en la firma pero no en el bound real

**8. Async en Rust (para extender examples/async.rs)**
- `futures::executor::LocalPool` y `LocalSpawner`
- Por que se usa `LocalPool` (no `Send`) en lugar de un executor multi-thread
- `task::LocalSpawnExt::spawn_local`

**9. SourcePawn (el lenguaje de scripting)**
- Sintaxis basica para escribir los docstrings de los natives
- `methodmap` para entender la convencion de nombres como `"Rust.Add"`
- `Handle` system desde la perspectiva del plugin
- Recursos: [SourcePawn Language Reference](https://wiki.alliedmods.net/Introduction_to_SourcePawn_1.7)

### Para Configurar el Entorno

**10. Cross-compilation 32-bit en Linux**
```bash
# Instalar target
rustup target add i686-unknown-linux-gnu
# Instalar gcc multilib
sudo apt install gcc-multilib  # Debian/Ubuntu
# O en Arch:
sudo pacman -S lib32-gcc-libs
```

**11. Servidor de SourceMod para Testing**
- Descargar SourceMod + Metamod:Source para tu juego objetivo
- Los `.so` generados van en `addons/sourcemod/extensions/`
- Los `.smx` de plugins van en `addons/sourcemod/plugins/`
- Comando de servidor: `sm exts list` para verificar carga, `sm exts unload/load nombre`
- Los errores de carga aparecen en `addons/sourcemod/logs/`
