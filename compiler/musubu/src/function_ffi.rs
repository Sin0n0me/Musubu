use alloc::{format, string::String, vec::Vec};
use core::{ffi::c_char, ptr, slice, str};
use musubu_engine::MusubuEngine;
use musubu_primitive::{Float, Integer, PrimitiveType, ToPrimitiveType, Value};

#[cfg(test)]
mod tests;

/// C-compatible scalar. Only the field selected by `kind` is read.
/// Tags are integers so an unknown tag can be rejected without constructing an invalid enum.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MusubuValue {
    pub kind: u32,
    pub signed_value: i64,
    pub unsigned_value: u64,
    pub float_value: f64,
}

impl MusubuValue {
    pub const UNIT: u32 = 0;
    pub const BOOL: u32 = 1;
    pub const I8: u32 = 2;
    pub const I16: u32 = 3;
    pub const I32: u32 = 4;
    pub const I64: u32 = 5;
    pub const U8: u32 = 6;
    pub const U16: u32 = 7;
    pub const U32: u32 = 8;
    pub const U64: u32 = 9;
    pub const F32: u32 = 10;
    pub const F64: u32 = 11;

    fn to_value(self) -> Result<Value, String> {
        let out_of_range = || String::from("scalar value is out of range for its kind");
        let integer = match self.kind {
            Self::UNIT => return Ok(Value::Unit),
            Self::BOOL if self.unsigned_value < 2 => {
                return Ok(Value::Bool(self.unsigned_value == 1));
            }
            Self::BOOL => return Err(String::from("bool must be 0 or 1")),
            Self::I8 => Integer::Int8(self.signed_value.try_into().map_err(|_| out_of_range())?),
            Self::I16 => Integer::Int16(self.signed_value.try_into().map_err(|_| out_of_range())?),
            Self::I32 => Integer::Int32(self.signed_value.try_into().map_err(|_| out_of_range())?),
            Self::I64 => Integer::Int64(self.signed_value),
            Self::U8 => Integer::Uint8(self.unsigned_value.try_into().map_err(|_| out_of_range())?),
            Self::U16 => {
                Integer::Uint16(self.unsigned_value.try_into().map_err(|_| out_of_range())?)
            }
            Self::U32 => {
                Integer::Uint32(self.unsigned_value.try_into().map_err(|_| out_of_range())?)
            }
            Self::U64 => Integer::Uint64(self.unsigned_value),
            Self::F32 => {
                let value = self.float_value as f32;
                if self.float_value.is_finite() && !value.is_finite() {
                    return Err(out_of_range());
                }
                return Ok(Value::Float(Float::Float32(value)));
            }
            Self::F64 => return Ok(Value::Float(Float::Float64(self.float_value))),
            _ => return Err(format!("unknown scalar kind {}", self.kind)),
        };
        Ok(Value::Integer(integer))
    }

    fn from_value(value: Value) -> Result<Self, String> {
        let mut result = Self::default();
        match value {
            Value::Unit => (),
            Value::Bool(value) => {
                result.kind = Self::BOOL;
                result.unsigned_value = u64::from(value);
            }
            Value::Integer(value) => match value {
                Integer::Int8(n) => {
                    result.kind = Self::I8;
                    result.signed_value = i64::from(n);
                }
                Integer::Int16(n) => {
                    result.kind = Self::I16;
                    result.signed_value = i64::from(n);
                }
                Integer::Int32(n) => {
                    result.kind = Self::I32;
                    result.signed_value = i64::from(n);
                }
                Integer::Int64(n) => {
                    result.kind = Self::I64;
                    result.signed_value = n;
                }
                Integer::Uint8(n) => {
                    result.kind = Self::U8;
                    result.unsigned_value = u64::from(n);
                }
                Integer::Uint16(n) => {
                    result.kind = Self::U16;
                    result.unsigned_value = u64::from(n);
                }
                Integer::Uint32(n) => {
                    result.kind = Self::U32;
                    result.unsigned_value = u64::from(n);
                }
                Integer::Uint64(n) => {
                    result.kind = Self::U64;
                    result.unsigned_value = n;
                }
            },
            Value::Float(Float::Float32(n)) => {
                result.kind = Self::F32;
                result.float_value = f64::from(n);
            }
            Value::Float(Float::Float64(n)) => {
                result.kind = Self::F64;
                result.float_value = n;
            }
            _ => {
                return Err(String::from(
                    "FFI supports only numeric, bool and unit values",
                ));
            }
        }
        Ok(result)
    }
}

fn supported(ty: &PrimitiveType) -> bool {
    matches!(
        ty,
        PrimitiveType::Unit
            | PrimitiveType::Boolean
            | PrimitiveType::Integer {
                byte: 1 | 2 | 4 | 8,
                ..
            }
            | PrimitiveType::Float { byte: 4 | 8 }
    )
}

fn execute(
    engine: &MusubuEngine,
    name: &str,
    arguments: &[MusubuValue],
) -> Result<MusubuValue, String> {
    let qualified = format!("musubu::{name}");
    let id = engine
        .get_function_id(&qualified)
        .ok_or_else(|| format!("undefined function `{name}`"))?;
    let function = engine
        .get_function(id)
        .ok_or_else(|| format!("undefined function `{name}`"))?;
    if !supported(&function.return_type) || function.parameters.iter().any(|ty| !supported(ty)) {
        return Err(String::from(
            "FFI supports only numeric, bool and unit function signatures",
        ));
    }
    if arguments.len() != function.parameters.len() {
        return Err(format!(
            "function `{name}` expects {} arguments, found {}",
            function.parameters.len(),
            arguments.len()
        ));
    }
    let args = arguments
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            let value = arg
                .to_value()
                .map_err(|error| format!("argument {index}: {error}"))?;
            if value.to_type() != function.parameters[index] {
                return Err(format!(
                    "argument {index}: expected {:?}, found {:?}",
                    function.parameters[index],
                    value.to_type()
                ));
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let value = engine
        .run_function(id, args)
        .map_err(|error| format!("VM error: {error:?}"))?
        .unwrap_or(Value::Unit);
    if value.to_type() != function.return_type {
        return Err(String::from(
            "VM returned a value that does not match the declared return type",
        ));
    }
    MusubuValue::from_value(value)
}

/// Calls a compiled function by its source name. Writes `output` only on success.
/// Clears the previous runtime error on every call with a non-null engine.
///
/// # Safety
/// The engine must be live and exclusively accessible. Input ranges must be valid,
/// aligned and initialized for their types. Output must be writable and aligned.
/// Input/output ranges must not alias engine storage. Arguments may be null only
/// when count is zero. Calls on one engine must not overlap.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn call_function(
    engine: *mut MusubuEngine,
    name: *const c_char,
    name_length: usize,
    arguments: *const MusubuValue,
    argument_count: usize,
    output: *mut MusubuValue,
) -> bool {
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return false;
    };
    engine.clear_runtime_error();
    let result = (|| {
        if name.is_null() || output.is_null() || (arguments.is_null() && argument_count != 0) {
            return Err(String::from("invalid null pointer in function call"));
        }
        if name_length > isize::MAX as usize
            || argument_count > isize::MAX as usize / size_of::<MusubuValue>()
        {
            return Err(String::from("function call input length is too large"));
        }
        let name = unsafe { slice::from_raw_parts(name.cast::<u8>(), name_length) };
        let name =
            str::from_utf8(name).map_err(|_| String::from("function name is not valid UTF-8"))?;
        let arguments = if argument_count == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(arguments, argument_count) }
        };
        execute(engine, name, arguments)
    })();
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

/// Returns borrowed UTF-8 bytes without a NUL terminator. Null means no error.
/// The bytes remain valid until the next call_function or engine destruction.
///
/// # Safety
/// Non-null engine must be live. Non-null length must be writable, aligned, and
/// must not alias the engine. Do not mutate the engine while reading the bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_runtime_error(
    engine: *const MusubuEngine,
    length: *mut usize,
) -> *const c_char {
    if length.is_null() {
        return ptr::null();
    }
    unsafe {
        *length = 0;
    }
    let Some(engine) = (unsafe { engine.as_ref() }) else {
        return ptr::null();
    };
    let error = engine.runtime_error();
    if error.is_empty() {
        return ptr::null();
    }
    unsafe {
        *length = error.len();
    }
    error.as_ptr().cast()
}
