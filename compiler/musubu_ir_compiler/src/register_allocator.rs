use alloc::vec::Vec;
use musubu_ir::Register;

#[derive(Debug)]
pub(crate) struct RegisterAllocator {
    register_size: usize, // レジスタの最大値を決めるために使用
    current_register: usize,
    check_points: Vec<usize>,
}

impl RegisterAllocator {
    const INITIAL_REGISTER: usize = 0;

    pub fn new() -> Self {
        Self {
            register_size: 0,
            current_register: Self::INITIAL_REGISTER,
            check_points: Vec::new(),
        }
    }

    pub fn alloc(&mut self) -> Register {
        let reg = self.current_register;
        self.current_register += 1;

        if self.register_size < self.current_register {
            self.register_size = self.current_register;
        }

        Register(reg)
    }

    pub fn enter_block(&mut self) {
        self.check_points.push(self.current_register);
    }

    pub fn exit_block(&mut self) {
        if let Some(reg) = self.check_points.pop() {
            self.current_register = reg;
        }
    }

    pub fn get_size(&self) -> usize {
        self.register_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequential_allocations_are_unique() {
        let mut allocator = RegisterAllocator::new();
        assert_eq!(allocator.get_size(), 0);
        for expected in 0..32 {
            assert_eq!(allocator.alloc().0, expected);
        }
        assert_eq!(allocator.get_size(), 32);
    }

    #[test]
    fn sibling_blocks_reuse_temporaries_without_shrinking_peak() {
        let mut allocator = RegisterAllocator::new();
        allocator.alloc();
        allocator.enter_block();
        assert_eq!(allocator.alloc().0, 1);
        assert_eq!(allocator.alloc().0, 2);
        allocator.exit_block();
        allocator.enter_block();
        assert_eq!(allocator.alloc().0, 1);
        allocator.exit_block();
        assert_eq!(allocator.get_size(), 3);
        assert_eq!(allocator.alloc().0, 1);
    }

    #[test]
    fn nested_checkpoints_restore_in_lifo_order() {
        let mut allocator = RegisterAllocator::new();
        allocator.enter_block();
        allocator.alloc();
        allocator.enter_block();
        allocator.alloc();
        allocator.exit_block();
        assert_eq!(allocator.alloc().0, 1);
        allocator.exit_block();
        assert_eq!(allocator.alloc().0, 0);
        assert_eq!(allocator.get_size(), 2);
    }

    #[test]
    fn empty_and_unmatched_exits_do_not_rewind_live_registers() {
        let mut allocator = RegisterAllocator::new();
        allocator.alloc();
        allocator.exit_block();
        allocator.enter_block();
        allocator.exit_block();
        assert_eq!(allocator.alloc().0, 1);
    }
}
