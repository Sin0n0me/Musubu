extern crate std;

use super::*;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::time::Instant;

struct CountingAllocator;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn measure(label: &str, mut operation: impl FnMut()) {
    const ITERATIONS: usize = 2000;
    operation();
    let start = Instant::now();
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    for _ in 0..ITERATIONS {
        operation();
    }
    let allocations = ALLOCATIONS.load(Ordering::Relaxed) - before;
    std::println!(
        "{label}: {} ns/call, {} allocations/call",
        start.elapsed().as_nanos() / ITERATIONS as u128,
        allocations / ITERATIONS
    );
}

/// Run alone with --release -- --ignored --nocapture --test-threads=1.
/// Timings are observations, not pass/fail thresholds; counts include result release.
#[test]
#[ignore = "manual isolated performance measurement"]
fn ffi_performance() {
    let mut engine = engine(
        "fn empty() {} fn scalar(x: i32) -> i32 { x + 1 } fn matrix(m: mat4x4) -> mat4x4 { m * 2.0 } enum Inner { V(vec3i32) } enum Outer { V(Inner) } fn nested(x: Outer) -> Outer { x }",
    );
    let scalar = scalar(&mut engine, integer(7));
    let vector = vector(&mut engine, &[integer(1), integer(2), integer(3)]);
    let inner = enumeration(&mut engine, "Inner", "V", &[&vector]);
    let nested = enumeration(&mut engine, "Outer", "V", &[&inner]);
    let data = [1.0_f32; 16];
    let matrix = receive(&mut engine, |e, out| unsafe {
        value_create_matrix_f32(e, data.as_ptr(), 4, 4, out)
    })
    .unwrap();
    for (name, args) in [
        ("empty", vec![]),
        ("scalar", pointers(&[&scalar])),
        ("matrix", pointers(&[&matrix])),
        ("nested", pointers(&[&nested])),
    ] {
        let id = engine.get_function_id(&format!("musubu::{name}")).unwrap();
        measure(&format!("{name}/name"), || {
            black_box(
                receive(&mut engine, |e, out| unsafe {
                    call_function_value(
                        e,
                        name.as_ptr().cast(),
                        name.len(),
                        args.as_ptr(),
                        args.len(),
                        out,
                    )
                })
                .unwrap(),
            );
        });
        measure(&format!("{name}/id"), || {
            black_box(
                receive(&mut engine, |e, out| unsafe {
                    call_function_value_by_id(e, id, args.as_ptr(), args.len(), out)
                })
                .unwrap(),
            );
        });
    }
    let mut output = [0.0_f32; 16];
    measure("matrix/copy", || {
        assert!(unsafe { value_copy_matrix_f32(&mut engine, matrix.0, output.as_mut_ptr(), 4, 4) });
        black_box(output);
    });
}
