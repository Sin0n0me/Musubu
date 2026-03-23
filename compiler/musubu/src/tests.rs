#[cfg(test)]
mod tests {
    use crate::compiler::*;
    use musubu_primitive::*;
    use musubu_vm::VM;
    use nalgebra::Matrix4;

    #[test]
    fn test_full() {
        assert!(compile(
            "
            fn main(input: matrix) -> matrix {

                let translate = matrix(
                    1.0, 0.0, 0.0, 2.0,
                    0.0, 1.0, 0.0, 5.0,
                    0.0, 0.0, 1.0, 0.0,
                    0.0, 0.0, 0.0, 1.0,
                );

                return input * translate;
        }
        "
        ));

        // デモ用
        let args = vec![Value::Matrix(Matrix::Matrix4(Matrix4::identity()))];
        let Some(value) = VM::new().run_function(0, args) else {
            panic!();
        };

        panic!("{value:?}");
    }
}
