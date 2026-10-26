use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidIR {
    ArgumentRegisters,
    Register { instruction: usize, register: usize },
    Jump { instruction: usize, target: usize },
}

impl CompiledFunction {
    /// Checks memory-safety invariants once, before the function enters a cache.
    /// Operand types and referenced function IDs are checked by the VM at runtime.
    pub fn validate(&self) -> Result<(), InvalidIR> {
        if self.parameters.len() > self.registers {
            return Err(InvalidIR::ArgumentRegisters);
        }
        for (ip, instruction) in self.code.iter().enumerate() {
            let check = |r: Register| {
                if r.0 < self.registers {
                    Ok(())
                } else {
                    Err(InvalidIR::Register {
                        instruction: ip,
                        register: r.0,
                    })
                }
            };
            let jump = |target: usize| {
                // A jump to the end is the existing implicit return convention.
                if target <= self.code.len() {
                    Ok(())
                } else {
                    Err(InvalidIR::Jump {
                        instruction: ip,
                        target,
                    })
                }
            };
            for register in instruction.registers() {
                check(register)?;
            }
            match instruction {
                Instruction::Jump { target } | Instruction::JumpIfFalse { target, .. } => {
                    jump(*target)?
                }
                Instruction::IterNext { exhausted, .. } => jump(*exhausted)?,
                _ => (),
            }
        }
        Ok(())
    }
}

impl Instruction {
    fn registers(&self) -> Vec<Register> {
        use Instruction::*;
        match self {
            LoadIndex { dst, parent, index } => alloc::vec![*dst, *parent, *index],
            StoreIndex {
                target,
                path,
                value,
            } => {
                let mut registers = alloc::vec![*target, *value];
                registers.extend(path);
                registers
            }
            MakeEnum { dst, fields, .. }
            | MakeStruct { dst, fields, .. }
            | MakeArray {
                dst,
                elements: fields,
                ..
            } => {
                let mut registers = alloc::vec![*dst];
                registers.extend(fields);
                registers
            }
            IsVariant { dst, value, .. } | RepeatArray { dst, value, .. } => {
                alloc::vec![*dst, *value]
            }
            LoadField { dst, parent, .. } => alloc::vec![*dst, *parent],
            StoreField { target, value, .. } => alloc::vec![*target, *value],
            MakeRange {
                dst, start, end, ..
            } => alloc::vec![*dst, *start, *end],
            IterInit { iterator, iterable } => alloc::vec![*iterator, *iterable],
            IterDrop { iterator } => alloc::vec![*iterator],
            IterNext { iterator, dst, .. } => alloc::vec![*iterator, *dst],
            LoadConst { dst, .. } => alloc::vec![*dst],
            Move { dst, src } => alloc::vec![*dst, *src],
            BinOp { dst, lhs, rhs, .. } | Cmp { dst, lhs, rhs, .. } => {
                alloc::vec![*dst, *lhs, *rhs]
            }
            JumpIfFalse { cond, .. } => alloc::vec![*cond],
            Call { dst, args, .. } => {
                let mut registers = args.clone();
                registers.extend(dst);
                registers
            }
            Return { value } => value.iter().copied().collect(),
            Jump { .. } | Unreachable => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function(instruction: Instruction) -> CompiledFunction {
        CompiledFunction {
            code: alloc::vec![instruction],
            registers: 1,
            parameters: Vec::new(),
            return_type: PrimitiveType::Unit,
        }
    }

    #[test]
    fn validates_registers_jumps_and_argument_storage() {
        let mut f = function(Instruction::Return { value: None });
        assert!(f.validate().is_ok());
        f.parameters = alloc::vec![PrimitiveType::Unit; 2];
        assert_eq!(f.validate(), Err(InvalidIR::ArgumentRegisters));
        for inst in [
            Instruction::Return {
                value: Some(Register(1)),
            },
            Instruction::Call {
                dst: None,
                func: 0,
                args: alloc::vec![Register(1)],
            },
            Instruction::StoreIndex {
                target: Register(0),
                path: alloc::vec![Register(1)],
                value: Register(0),
            },
        ] {
            assert!(matches!(
                function(inst).validate(),
                Err(InvalidIR::Register { .. })
            ));
        }
        assert!(function(Instruction::Jump { target: 1 }).validate().is_ok());
        assert!(matches!(
            function(Instruction::Jump { target: 2 }).validate(),
            Err(InvalidIR::Jump { .. })
        ));
        assert!(matches!(
            function(Instruction::IterNext {
                iterator: Register(0),
                dst: Register(0),
                exhausted: 2
            })
            .validate(),
            Err(InvalidIR::Jump { .. })
        ));
    }
}
