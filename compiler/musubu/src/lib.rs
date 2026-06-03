// TODO
// #![no_std]

extern crate alloc;

#[cfg(test)]
mod tests;

use musubu_engine::MusubuEngine;
use std::os::raw::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

#[unsafe(no_mangle)]
pub extern "C" fn init(output: *mut *mut MusubuEngine) -> bool {
    let result = catch_unwind(AssertUnwindSafe(|| {
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
    }));

    result.unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn uninit(engine: *mut MusubuEngine) {
    let _result = catch_unwind(AssertUnwindSafe(|| {
        if engine.is_null() {
            return;
        }

        // Boxに戻した時点で所有権をRustに戻す
        unsafe {
            drop(Box::from_raw(engine));
        }
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn compile(engine: *mut MusubuEngine, code_ptr: *const c_char, len: usize) -> bool {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if engine.is_null() {
            return false;
        }
        let engine = unsafe { &mut *engine };

        if code_ptr.is_null() {
            return false;
        }

        let bytes = unsafe { std::slice::from_raw_parts(code_ptr as *const u8, len) };
        let code = match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => return false,
        };

        musubu_driver::compile(engine, code)
    }));

    result.unwrap_or(false)
}

// TODO 以下2つの中身の実装
#[unsafe(no_mangle)]
pub extern "C" fn call_function() {}

// 初期化せずに使用する場合
// キャッシュを使用しないので毎回
// 字句解析->構文解析->意味解析->脱糖->命令化
// の流れが発生するので重い
pub extern "C" fn run_script() {}
