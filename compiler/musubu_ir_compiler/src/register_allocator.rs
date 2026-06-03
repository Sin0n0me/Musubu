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
