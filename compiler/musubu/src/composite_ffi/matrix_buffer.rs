use super::*;

fn dimensions<T>(columns: usize, rows: usize) -> Result<usize, String> {
    if columns == 0 || rows == 0 || u32::try_from(columns).is_err() || u32::try_from(rows).is_err()
    {
        return Err(String::from(
            "matrix dimensions must be between 1 and u32::MAX",
        ));
    }
    // A packed matrix contains columns * rows elements.
    columns
        .checked_mul(rows)
        .filter(|count| *count <= isize::MAX as usize / size_of::<T>())
        .ok_or_else(|| String::from("matrix buffer length is too large"))
}

macro_rules! matrix_buffer_api {
    ($create:ident, $copy:ident, $scalar:ty, $kind:ident) => {
        /// Copies a packed column-major buffer into an owned matrix handle.
        /// # Safety
        /// data must contain columns * rows initialized, aligned scalar elements.
        /// Other pointers and exclusive engine access follow value_create_scalar.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $create(
            engine: *mut MusubuEngine,
            data: *const $scalar,
            columns: usize,
            rows: usize,
            output: *mut *mut MusubuValueHandle,
        ) -> bool {
            unsafe {
                write_result(engine, output, |_| {
                    let count = dimensions::<$scalar>(columns, rows)?;
                    let data = input(data, count)?;
                    let columns = data
                        .chunks_exact(rows)
                        .map(|column| {
                            let components = column
                                .iter()
                                .map(|v| MusubuValue {
                                    kind: MusubuValue::$kind,
                                    float_value: f64::from(*v),
                                    ..MusubuValue::default()
                                })
                                .collect::<Vec<_>>();
                            HostValue::vector(&components)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(MusubuValueHandle::owned(HostValue::matrix(columns)?))
                })
            }
        }

        /// Copies a matrix into a packed column-major caller buffer.
        /// Shape and scalar type must match exactly. Failure leaves data unchanged.
        /// # Safety
        /// data must be writable for columns * rows aligned scalar elements and
        /// must not alias value or engine. value must be live; engine exclusive.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $copy(
            engine: *mut MusubuEngine,
            value: *const MusubuValueHandle,
            data: *mut $scalar,
            columns: usize,
            rows: usize,
        ) -> bool {
            let mut completed = ();
            unsafe {
                write_result(engine, &mut completed, |_| {
                    let count = dimensions::<$scalar>(columns, rows)?;
                    if data.is_null() {
                        return Err(String::from("matrix output buffer must not be null"));
                    }
                    let HostValue::Plain(Value::Matrix(value)) = handle(value)? else {
                        return Err(String::from("value is not a matrix"));
                    };
                    if value.column_count() != columns {
                        return Err(String::from("matrix column count or value kind mismatch"));
                    }
                    // Validate all components before allocating the output staging buffer.
                    // Keep writes transactional even if an invalid component is encountered.
                    for column in 0..columns {
                        if value.column_len(column) != Some(rows) {
                            return Err(String::from("matrix row count mismatch"));
                        }
                        for row in 0..rows {
                            let component = value
                                .component(column, row)
                                .ok_or_else(|| String::from("invalid matrix component"))?;
                            let component = MusubuValue::from_value(component)?;
                            if component.kind != MusubuValue::$kind {
                                return Err(String::from("matrix scalar type mismatch"));
                            }
                        }
                    }
                    let mut elements = Vec::with_capacity(count);
                    for column in 0..columns {
                        for row in 0..rows {
                            let component = value
                                .component(column, row)
                                .ok_or_else(|| String::from("invalid matrix component"))?;
                            let component = MusubuValue::from_value(component)?;
                            // Column-major offset = column index * rows + row index.
                            elements.push(component.float_value as $scalar);
                        }
                    }
                    ptr::copy_nonoverlapping(elements.as_ptr(), data, count);
                    Ok(())
                })
            }
        }
    };
}

matrix_buffer_api!(value_create_matrix_f32, value_copy_matrix_f32, f32, F32);
matrix_buffer_api!(value_create_matrix_f64, value_copy_matrix_f64, f64, F64);
