use musubu_primitive::*;
use nalgebra::Matrix4;

pub fn make_matrix_4x4_from_16_args(args: Vec<Value>) -> Option<Value> {
    if args.len() != 16 {
        // TODO: Err
        return None;
    }

    let iter = args.into_iter().map(|v| {
        let Value::Float(Float::Float32(f)) = v else {
            unreachable!();
        };
        f
    });

    Some(Value::Matrix(Matrix::Matrix4(Matrix4::from_iterator(iter))))
}
