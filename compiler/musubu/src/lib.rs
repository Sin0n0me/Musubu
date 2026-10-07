#![no_std]

extern crate alloc;

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use alloc::string::ToString;
use core::ffi::c_char;
use core::ptr;
use core::slice::from_raw_parts;
use core::str::from_utf8;
use musubu_engine::MusubuEngine;

#[unsafe(no_mangle)]
pub extern "C" fn init(output: *mut *mut MusubuEngine) -> bool {
    if output.is_null() {
        return false;
    }

    // Box を生ポインタ化して所有権を FFI 側へ渡す
    let engine = Box::new(MusubuEngine::new());
    let raw = Box::into_raw(engine);
    unsafe {
        ptr::write(output, raw);
    }

    true
}

#[unsafe(no_mangle)]
pub extern "C" fn uninit(engine: *mut MusubuEngine) {
    if engine.is_null() {
        return;
    }

    // Boxに戻した時点で所有権をRustに戻す
    unsafe {
        drop(Box::from_raw(engine));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn compile(engine: *mut MusubuEngine, code_ptr: *const c_char, len: usize) -> bool {
    if engine.is_null() {
        return false;
    }
    let engine = unsafe { &mut *engine };
    engine.clear_compile_error();

    if code_ptr.is_null() {
        engine.set_compile_error("error: source pointer must not be null\n".to_string());
        return false;
    }

    let bytes = unsafe { from_raw_parts(code_ptr as *const u8, len) };
    let code = match from_utf8(bytes) {
        Ok(s) => s,
        Err(error) => {
            engine.set_compile_error(alloc::format!(
                "error: source is not valid UTF-8: {error}\n"
            ));
            return false;
        }
    };

    musubu_driver::compile(engine, code)
}

/// Compiles UTF-8 source using the supplied display path (normally project-relative).
///
/// # Safety
/// A non-null engine must be live and exclusively accessible. Both non-null input
/// byte ranges must be readable, at most `isize::MAX` bytes, and must not alias
/// the engine's storage. Calls on the same engine must not overlap.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compile_with_filename(
    engine: *mut MusubuEngine,
    code_ptr: *const c_char,
    code_len: usize,
    filename_ptr: *const c_char,
    filename_len: usize,
) -> bool {
    if engine.is_null() {
        return false;
    }
    let engine = unsafe { &mut *engine };
    engine.clear_compile_error();
    if code_ptr.is_null() || filename_ptr.is_null() {
        engine.set_compile_error(
            "error: source and filename pointers must not be null\n".to_string(),
        );
        return false;
    }
    let code = unsafe { from_raw_parts(code_ptr.cast::<u8>(), code_len) };
    let filename = unsafe { from_raw_parts(filename_ptr.cast::<u8>(), filename_len) };
    let (code, filename) = match (from_utf8(code), from_utf8(filename)) {
        (Ok(code), Ok(filename)) => (code, filename),
        (Err(error), _) => {
            engine.set_compile_error(alloc::format!(
                "error: source is not valid UTF-8: {error}\n"
            ));
            return false;
        }
        (_, Err(error)) => {
            engine.set_compile_error(alloc::format!(
                "error: filename is not valid UTF-8: {error}\n"
            ));
            return false;
        }
    };
    musubu_driver::compile_with_filename(engine, filename, code)
}

/// Returns UTF-8 diagnostic bytes, excluding any terminator (the buffer is NOT NUL-terminated).
/// The borrowed pointer remains valid until the engine is mutated or destroyed.
/// Returns null and writes zero length when no diagnostic exists or the engine is null.
/// `len` must be a writable pointer; a null `len` returns null.
///
/// # Safety
/// Non-null pointers must be aligned and valid for their types. `len` must not
/// alias the engine. The engine must remain live and unmodified while the
/// returned byte range is read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_compile_error(
    engine: *const MusubuEngine,
    len: *mut usize,
) -> *const c_char {
    if len.is_null() {
        return ptr::null();
    }
    unsafe {
        *len = 0;
    }
    if engine.is_null() {
        return ptr::null();
    }
    let error = unsafe { &*engine }.compile_error();
    if error.is_empty() {
        return ptr::null();
    }
    unsafe {
        *len = error.len();
    }
    error.as_ptr().cast::<c_char>()
}

// TODO 以下2つの中身の実装
#[unsafe(no_mangle)]
pub extern "C" fn call_function() {}

// 初期化せずに使用する場合
// キャッシュを使用しないので毎回
// 字句解析->構文解析->意味解析->脱糖->命令化
// の流れが発生するので重い
pub extern "C" fn run_script() {}
