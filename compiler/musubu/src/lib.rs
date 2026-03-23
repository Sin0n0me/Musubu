mod compiler;

#[cfg(test)]
mod tests;

use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::slice;

use musubu_primitive::*;
use musubu_vm::VM;
use nalgebra::Matrix4;

#[unsafe(no_mangle)]
pub extern "C" fn compile(code_ptr: *const c_char, len: usize) -> bool {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if code_ptr.is_null() {
            return false;
        }

        let bytes = unsafe { std::slice::from_raw_parts(code_ptr as *const u8, len) };
        let code = match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => return false,
        };

        compiler::compile(code)
    }));

    result.unwrap_or(false)
}

// デモ用
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
