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

#[unsafe(no_mangle)]
pub extern "C" fn call_function() {}

// 初期化せずに使用する場合
// キャッシュを使用しないので毎回
// 字句解析->構文解析->意味解析->脱糖->命令化
// の流れが発生するので重い
pub extern "C" fn run_script() {}

// デモ用
/*
const MAT_ELEM_COUNT: usize = 16;
#[unsafe(no_mangle)]
pub extern "C" fn run_script(matrix_ptr: *mut f32) -> bool {
    // panicを外に出さないようにする
    let result = catch_unwind(AssertUnwindSafe(|| {
        if matrix_ptr.is_null() {
            return false;
        }

        let matrix_slice = unsafe { slice::from_raw_parts_mut(matrix_ptr, MAT_ELEM_COUNT) };

        let mat = Matrix4::<f32>::from_column_slice(matrix_slice);
        let args = vec![Value::Matrix(Matrix::Matrix4(mat))];
        let vm = VM::new();
        let Some(Value::Matrix(Matrix::Matrix4(result))) = vm.run_function(0, args) else {
            return false;
        };

        // 結果の書き戻し
        matrix_slice.copy_from_slice(result.as_slice());

        true
    }));

    match result {
        Ok(v) => v,
        Err(_) => false,
    }
}
 * */
