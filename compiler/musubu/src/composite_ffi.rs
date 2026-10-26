mod matrix_buffer;
#[cfg(test)]
mod tests;
mod value;
use crate::MusubuValue;
use alloc::{boxed::Box, format, string::String, vec::Vec};
use core::{ffi::c_char, ptr, slice, str};
pub use matrix_buffer::*;
pub use musubu_engine::FunctionToken as MusubuFunctionToken;
use musubu_engine::MusubuEngine;
use musubu_primitive::{ToPrimitiveType, Value};
use value::HostValue;
pub use value::MusubuValueHandle;

unsafe fn input<'a, T>(pointer: *const T, count: usize) -> Result<&'a [T], String> {
    // Maximum element count = isize::MAX / element size; a slice cannot exceed that byte length.
    if count > isize::MAX as usize / size_of::<T>() {
        return Err(String::from("input length is too large"));
    }
    if count == 0 {
        return Ok(&[]);
    }
    if pointer.is_null() {
        return Err(String::from("input pointer must not be null"));
    }
    Ok(unsafe { slice::from_raw_parts(pointer, count) })
}

unsafe fn text<'a>(pointer: *const c_char, length: usize) -> Result<&'a str, String> {
    let bytes = unsafe { input(pointer.cast::<u8>(), length) }?;
    let text = str::from_utf8(bytes).map_err(|_| String::from("name is not valid UTF-8"))?;
    if text.is_empty() {
        return Err(String::from("name must not be empty"));
    }
    Ok(text)
}

unsafe fn handle<'a>(value: *const MusubuValueHandle) -> Result<&'a HostValue, String> {
    unsafe { value.as_ref() }
        .map(|v| &v.0)
        .ok_or_else(|| String::from("value must not be null"))
}

unsafe fn children(
    values: *const *const MusubuValueHandle,
    count: usize,
) -> Result<Vec<HostValue>, String> {
    unsafe { input(values, count) }?
        .iter()
        .map(|p| unsafe { handle(*p) }.cloned())
        .collect()
}

unsafe fn borrowed_children<'a>(
    values: *const *const MusubuValueHandle,
    count: usize,
) -> Result<Vec<&'a HostValue>, String> {
    unsafe { input(values, count) }?
        .iter()
        .map(|value| unsafe { handle(*value) })
        .collect()
}

// Validate output before allocating owned handles; every failure leaves it unchanged.
unsafe fn write_result<T>(
    engine: *mut MusubuEngine,
    output: *mut T,
    operation: impl FnOnce(&MusubuEngine) -> Result<T, String>,
) -> bool {
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return false;
    };
    engine.clear_runtime_error();
    let result = if output.is_null() {
        Err(String::from("output must not be null"))
    } else {
        operation(engine)
    };
    match result {
        Ok(value) => {
            unsafe {
                ptr::write(output, value);
            }
            true
        }
        Err(error) => {
            engine.set_runtime_error(error);
            false
        }
    }
}

/// Creates an owned scalar handle by copying the supplied scalar.
/// # Safety
/// All non-null pointers must be valid and aligned, output writable. Inputs/output
/// must not alias engine storage. Engine must be exclusively accessible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn value_create_scalar(
    engine: *mut MusubuEngine,
    scalar: *const MusubuValue,
    output: *mut *mut MusubuValueHandle,
) -> bool {
    unsafe {
        write_result(engine, output, |_| {
            let scalar = scalar
                .as_ref()
                .ok_or_else(|| String::from("scalar must not be null"))?;
            Ok(MusubuValueHandle::owned(HostValue::Plain(
                scalar.to_value()?,
            )))
        })
    }
}

/// Copies homogeneous numeric components into an owned vector handle.
/// # Safety
/// Same contract as value_create_scalar; components must contain count initialized values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn value_create_vector(
    engine: *mut MusubuEngine,
    components: *const MusubuValue,
    count: usize,
    output: *mut *mut MusubuValueHandle,
) -> bool {
    unsafe {
        write_result(engine, output, |_| {
            Ok(MusubuValueHandle::owned(HostValue::vector(input(
                components, count,
            )?)?))
        })
    }
}

/// Copies equal-shaped vector columns into an owned column-major matrix handle.
/// # Safety
/// Same contract as value_create_scalar. columns contains count pointers to live handles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn value_create_matrix(
    engine: *mut MusubuEngine,
    columns: *const *const MusubuValueHandle,
    count: usize,
    output: *mut *mut MusubuValueHandle,
) -> bool {
    unsafe {
        write_result(engine, output, |_| {
            Ok(MusubuValueHandle::owned(HostValue::matrix(children(
                columns, count,
            )?)?))
        })
    }
}

/// Copies an enum descriptor. Variant and field types are validated when called.
/// Fields are in source declaration order for both tuple and named variants.
/// # Safety
/// Same contract as value_create_scalar. Names are readable UTF-8 byte ranges;
/// fields contains count live handle pointers, or may be null when count is zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn value_create_enum(
    engine: *mut MusubuEngine,
    type_name: *const c_char,
    type_length: usize,
    variant: *const c_char,
    variant_length: usize,
    fields: *const *const MusubuValueHandle,
    count: usize,
    output: *mut *mut MusubuValueHandle,
) -> bool {
    unsafe {
        write_result(engine, output, |_| {
            Ok(MusubuValueHandle::owned(HostValue::Enum {
                name: String::from(text(type_name, type_length)?),
                variant: String::from(text(variant, variant_length)?),
                fields: children(fields, count)?,
            }))
        })
    }
}

fn resolve_function(engine: &MusubuEngine, name: &str) -> Result<usize, String> {
    engine
        .get_function_id(&format!("musubu::{name}"))
        .ok_or_else(|| format!("undefined function `{name}`"))
}

fn execute(
    engine: &MusubuEngine,
    id: usize,
    name: Option<&str>,
    args: &[&HostValue],
) -> Result<HostValue, String> {
    let function = engine
        .get_function(id)
        .ok_or_else(|| format!("undefined function ID {id}"))?;
    if !value::supported(&function.return_type)
        || function.parameters.iter().any(|ty| !value::supported(ty))
    {
        return Err(String::from(
            "unsupported FFI signature: expected scalar, enum, vec or mat types",
        ));
    }
    if args.len() != function.parameters.len() {
        let label = match name {
            Some(name) => format!("`{name}`"),
            None => format!("ID {id}"),
        };
        return Err(format!(
            "function {label} expects {} arguments, found {}",
            function.parameters.len(),
            args.len()
        ));
    }
    let args = args
        .iter()
        .zip(&function.parameters)
        .enumerate()
        .map(|(i, (arg, ty))| {
            arg.to_runtime(ty)
                .map_err(|error| format!("argument {i}: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = engine
        .run_function_diagnostic(id, args)?
        .unwrap_or(Value::Unit);
    if result.to_type() != function.return_type {
        return Err(String::from(
            "VM return type does not match the declared return type",
        ));
    }
    HostValue::from_runtime(result)
}

/// Calls a function with borrowed input handles and returns a new owned handle.
/// # Safety
/// Same contract as value_create_scalar. name is a UTF-8 byte range, arguments
/// contains count live handle pointers (may be null for zero). No handles are consumed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn call_function_value(
    engine: *mut MusubuEngine,
    name: *const c_char,
    name_length: usize,
    arguments: *const *const MusubuValueHandle,
    count: usize,
    output: *mut *mut MusubuValueHandle,
) -> bool {
    unsafe {
        write_result(engine, output, |engine| {
            let name = text(name, name_length)?;
            let result = execute(
                engine,
                resolve_function(engine, name)?,
                Some(name),
                &borrowed_children(arguments, count)?,
            )?;
            Ok(MusubuValueHandle::owned(result))
        })
    }
}

/// Resolves a source function name once for repeated calls by ID.
/// IDs are engine-local; resolve again after any compilation attempt.
/// # Safety
/// Same contract as value_create_scalar. name contains name_length UTF-8 bytes.
/// output must point to writable usize storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_function_id(
    engine: *mut MusubuEngine,
    name: *const c_char,
    name_length: usize,
    output: *mut usize,
) -> bool {
    unsafe {
        write_result(engine, output, |engine| {
            resolve_function(engine, text(name, name_length)?)
        })
    }
}

/// Calls a resolved function without string construction or name lookup.
/// IDs must be obtained from this engine since its last compilation attempt.
/// # Safety
/// Same pointer and ownership contract as call_function_value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn call_function_value_by_id(
    engine: *mut MusubuEngine,
    function_id: usize,
    arguments: *const *const MusubuValueHandle,
    count: usize,
    output: *mut *mut MusubuValueHandle,
) -> bool {
    unsafe {
        write_result(engine, output, |engine| {
            let result = execute(
                engine,
                function_id,
                None,
                &borrowed_children(arguments, count)?,
            )?;
            Ok(MusubuValueHandle::owned(result))
        })
    }
}

/// Resolves a name into an engine- and revision-specific token.
/// # Safety
/// Same contract as get_function_id, with writable MusubuFunctionToken output.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_function_token(
    engine: *mut MusubuEngine,
    name: *const c_char,
    name_length: usize,
    output: *mut MusubuFunctionToken,
) -> bool {
    unsafe {
        write_result(engine, output, |engine| {
            let name = text(name, name_length)?;
            engine
                .get_function_token(&format!("musubu::{name}"))
                .ok_or_else(|| format!("undefined function `{name}`"))
        })
    }
}

/// Calls a function token, rejecting other engines and replaced programs.
/// # Safety
/// Same contract as call_function_value. token must point to an initialized token.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn call_function_value_by_token(
    engine: *mut MusubuEngine,
    token: *const MusubuFunctionToken,
    arguments: *const *const MusubuValueHandle,
    count: usize,
    output: *mut *mut MusubuValueHandle,
) -> bool {
    unsafe {
        write_result(engine, output, |engine| {
            let token = token
                .as_ref()
                .ok_or_else(|| String::from("token must not be null"))?;
            let id = engine
                .token_function(*token)
                .ok_or_else(|| String::from("stale or foreign function token"))?;
            let result = execute(engine, id, None, &borrowed_children(arguments, count)?)?;
            Ok(MusubuValueHandle::owned(result))
        })
    }
}

/// Copies a scalar payload out of a handle. Aggregate handles are rejected.
/// # Safety
/// Same contract as value_create_scalar; value must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn value_get_scalar(
    engine: *mut MusubuEngine,
    value: *const MusubuValueHandle,
    output: *mut MusubuValue,
) -> bool {
    unsafe {
        write_result(engine, output, |_| match handle(value)? {
            HostValue::Plain(value) => MusubuValue::from_value(value.clone()),
            _ => Err(String::from("value is not a scalar")),
        })
    }
}

/// Returns an independent owned copy of a vector component, matrix column or enum field.
/// # Safety
/// Same contract as value_create_scalar; value must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn value_get_element(
    engine: *mut MusubuEngine,
    value: *const MusubuValueHandle,
    index: usize,
    output: *mut *mut MusubuValueHandle,
) -> bool {
    unsafe {
        write_result(engine, output, |_| {
            Ok(MusubuValueHandle::owned(handle(value)?.element(index)?))
        })
    }
}

/// Returns MUSUBU_INVALID for null. Scalar tags are shared with MusubuValue.
/// # Safety
/// A non-null value must be a live handle; it must not be freed during this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn value_get_kind(value: *const MusubuValueHandle) -> u32 {
    unsafe { handle(value) }
        .map(HostValue::kind)
        .unwrap_or(MusubuValue::INVALID)
}

/// Returns component/column/field count. Scalar and null handles return zero.
/// # Safety
/// Same contract as value_get_kind.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn value_get_length(value: *const MusubuValueHandle) -> usize {
    unsafe { handle(value) }.map(HostValue::len).unwrap_or(0)
}

unsafe fn enum_text(
    value: *const MusubuValueHandle,
    length: *mut usize,
    variant_name: bool,
) -> *const c_char {
    if length.is_null() {
        return ptr::null();
    }
    unsafe {
        *length = 0;
    }
    let Ok(HostValue::Enum { name, variant, .. }) = (unsafe { handle(value) }) else {
        return ptr::null();
    };
    let text = if variant_name { variant } else { name };
    unsafe {
        *length = text.len();
    }
    text.as_ptr().cast()
}

/// Borrows the enum type name as UTF-8 bytes, valid until value_free(value).
/// Null/non-enum returns null and zero length. Null length returns null.
/// # Safety
/// value must be null or live. Non-null length must be writable, aligned and must
/// not alias the value. Do not free the value while reading the borrowed bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn value_get_enum_type(
    value: *const MusubuValueHandle,
    length: *mut usize,
) -> *const c_char {
    unsafe { enum_text(value, length, false) }
}

/// Borrows the enum variant name; same lifetime and null behavior as value_get_enum_type.
/// # Safety
/// Same contract as value_get_enum_type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn value_get_enum_variant(
    value: *const MusubuValueHandle,
    length: *mut usize,
) -> *const c_char {
    unsafe { enum_text(value, length, true) }
}

/// Releases a handle and all its owned fields. Null is a no-op.
/// # Safety
/// A non-null value must be returned by this API, must not have been freed, and
/// must not be borrowed by another call. Each owned handle must be freed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn value_free(value: *mut MusubuValueHandle) {
    if !value.is_null() {
        unsafe {
            drop(Box::from_raw(value));
        }
    }
}
