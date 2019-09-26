pub mod memory;

use syntax::*;
use std::collections::HashMap;

#[derive(Debug)]
pub struct VM {
    pub memory: memory::Memory,
    frames: Vec<StackFrame>,
    fns: HashMap<String, Function>,
}

#[derive(Debug)]
struct StackFrame {
    sizes: HashMap<LocalId, usize>,
    locals: HashMap<LocalId, usize>,
    block: BlockId,
}

impl VM {
    pub fn new(program: Program) -> VM {
        let memory = memory::Memory::new();
        let frames = Vec::new();
        let fns: HashMap<String, Function> = program.fns.into_iter().map(|f| (f.name.text.clone(), f)).collect();
        
        VM {
            memory,
            frames,
            fns,
        }
    }
    
    pub fn run(&mut self) -> Option<usize> {
        let f = self.fns["main"].clone();
        
        self.frames.push(StackFrame {
            locals: HashMap::new(),
            sizes: f.bindings.iter().map(|b| (b.0, b.1.size())).collect(),
            block: BlockId(0),
        });
        
        if let Some(loc) = self.run_fn(f) {
            Some(self.memory.read_u32(loc) as usize)
        } else {
            None
        }
    }
    
    fn run_fn(&mut self, f: Function) -> Option<usize> {
        // init return memory
        let loc = self.memory.stack.len();
        
        self.frame_mut().locals.insert(f.bindings[0].0, loc);
        self.init(self.frame().sizes[&f.bindings[0].0]);
        
        loop {
            let block = self.block(&f);
            
            for stmt in block.statements {
                match stmt {
                    Statement::StorageLive(id) => {
                        let loc = self.memory.stack.len();
                        
                        self.frame_mut().locals.insert(id, loc);
                        self.init(self.frame().sizes[&id]);
                    },
                    Statement::StorageDead(id) => {
                        self.frame_mut().locals.remove(&id);
                        self.drop(self.frame().sizes[&id]);
                    },
                    Statement::Assign(place, value) => {
                        let (loc, size) = self.place(place);
                        let val = self.rvalue(value);
                        let bytes = val.to_le_bytes();
                        
                        for i in 0..size { self.memory.stack[loc + i] = bytes[i]; }
                    },
                }
            }
            
            match block.terminator {
                Terminator::Return => return Some(loc),
                Terminator::Unreachable => unreachable!(),
                Terminator::Goto(id) => self.frame_mut().block = id,
                Terminator::Abort => {
                    // StorageDead($0)
                    self.drop(self.frame().sizes[&f.bindings[0].0]);
                    
                    return None;
                },
                Terminator::Resume => {
                    
                },
                Terminator::Call(f, args, goto, fail) => {
                    let f = self.operand(f);
                    let f = self.fns.iter().nth(f.0 as usize).unwrap().1.clone();
                    let mut frame = StackFrame {
                        locals: HashMap::new(),
                        sizes: f.bindings.iter().map(|b| (b.0, b.1.size())).collect(),
                        block: BlockId(0),
                    };
                    
                    // init params
                    for ((id, ty), arg) in f.params.iter().zip(args.iter()) {
                        let size = ty.size();
                        let loc = self.memory.stack.len();
                        
                        frame.locals.insert(*id, loc);
                        self.init(size);
                        
                        let val = self.operand(arg.clone()).0;
                        let bytes = val.to_le_bytes();
                        
                        for i in 0..size { self.memory.stack[loc + i] = bytes[i]; }
                    }
                    
                    self.frames.push(frame);
                    
                    let val = self.run_fn(f);
                    
                    self.frames.pop().unwrap();
                    
                    match (goto, fail) {
                        (Some((place, next)), Some(fail)) => {
                            if let Some(val) = val {
                                let (loc, size) = self.place(place);
                                let bytes = val.to_le_bytes();
                                
                                for i in 0..size { self.memory.stack[loc + i] = bytes[i]; } 
                                
                                self.drop(size);
                                self.frame_mut().block = next;
                            } else {
                                self.frame_mut().block = fail;
                            }
                        },
                        (Some((place, next)), None) => {
                            if let Some(val) = val {
                                let (loc, size) = self.place(place);
                                let bytes = val.to_le_bytes();
                                
                                for i in 0..size { self.memory.stack[loc + i] = bytes[i]; } 
                                
                                self.drop(size);
                                self.frame_mut().block = next;
                            } else {
                                return None;
                            }
                        },
                        (None, Some(fail)) => {
                            if let None = val {
                                self.frame_mut().block = fail;
                            } else {
                                return Some(loc);
                            }
                        },
                        (None, None) => {
                            if let None = val {
                                return None;
                            } else {
                                return Some(loc);
                            }
                        }
                    }
                },
                Terminator::Assert(op, expected, success, fail) => {
                    let op = self.operand(op);
                    let val = self.memory.read_u8(op.0 as usize);
                    
                    if (val != 0) == expected {
                        self.frame_mut().block = success;
                    } else {
                        if let Some(fail) = fail {
                            self.frame_mut().block = fail;
                        } else {
                            return None;
                        }
                    }
                }
            }
        }
    }
    
    fn place(&mut self, p: Place) -> (usize, usize) {
        let (mut loc, mut size) = match p.base {
            PlaceBase::Local(id) => (self.frame().locals[&id], self.frame().sizes[&id])
        };
        
        for proj in p.projection.into_iter().rev() {
            match proj {
                PlaceElem::Field(i) => loc += i,
                PlaceElem::Deref => loc = self.memory.read_u32(loc) as usize,
            }
        }
        
        (loc, size)
    }
    
    fn rvalue(&mut self, v: RValue) -> u64 {
        match v {
            RValue::Use(op) => self.operand(op).0,
            RValue::Binary(op, lhs, rhs) => {
                let lhs = self.operand(lhs);
                let rhs = self.operand(rhs);
                let lhs = self.memory.read(lhs.0 as usize, lhs.1);
                let rhs = self.memory.read(rhs.0 as usize, rhs.1);
                
                match op {
                    BinOp::Add => lhs + rhs,
                    BinOp::Sub => lhs - rhs,
                    BinOp::Mul => lhs * rhs,
                    BinOp::Div => lhs / rhs,
                    BinOp::Mod => lhs % rhs,
                    BinOp::Lt => (lhs < rhs) as u64,
                    BinOp::Le => (lhs <= rhs) as u64,
                    BinOp::Gt => (lhs > rhs) as u64,
                    BinOp::Ge => (lhs >= rhs) as u64,
                    BinOp::Eq => (lhs == rhs) as u64,
                    BinOp::Ne => (lhs != rhs) as u64,
                    BinOp::BitAnd => lhs & rhs,
                    BinOp::BitOr => lhs | rhs,
                    BinOp::BitXor => lhs ^ rhs,
                    BinOp::Shl => lhs << rhs,
                    BinOp::Shr => lhs >> rhs,
                }
            },
            _ => unimplemented!()
        }
    }
    
    fn operand(&mut self, o: Operand) -> (u64, usize) {
        match o {
            Operand::Constant(c) => self.constant(c),
            Operand::Copy(p) => { let p = self.place(p); (p.0 as u64, p.1) },
            Operand::Move(p) => unimplemented!()
        }
    }
    
    fn constant(&mut self, c: Constant) -> (u64, usize) {
        match c {
            Constant::Int(v, IntTy::I8) => (v as u64, 1),
            Constant::Int(v, IntTy::I16) => (v as u64, 2),
            Constant::Int(v, IntTy::I32) => (v as u64, 4),
            Constant::Int(v, IntTy::I64) => (v as u64, 8),
            Constant::UInt(v, UIntTy::U8) => (v, 1),
            Constant::UInt(v, UIntTy::U16) => (v, 2),
            Constant::UInt(v, UIntTy::U32) => (v, 4),
            Constant::UInt(v, UIntTy::U64) => (v, 8),
            Constant::Float(v, FloatTy::F32) => (v.to_bits(), 4),
            Constant::Float(v, FloatTy::F64) => (v.to_bits(), 8),
            Constant::Bool(b) => (b as u64, 1),
            Constant::Item(id) => {
                for (i, (name, _)) in self.fns.iter().enumerate() {
                    if name == &id.text { return (i as u64, 0); }
                }
                
                panic!("unknown symbol")
            },
            _ => unimplemented!()
        }
    }
    
    fn init(&mut self, size: usize) {
        for _ in 0..size { self.memory.stack.push(0); }
    }
    
    fn drop(&mut self, size: usize) {
        for _ in 0..size { self.memory.stack.pop().expect("stack underflow"); }
    }
    
    fn block(&self, f: &Function) -> BasicBlock {
        if let Some(b) = f.blocks.iter().find(|b| b.id == self.frame().block) {
            b.clone()
        } else {
            panic!("undefined block {}", self.frame().block);
        }
    }
    
    fn frame(&self) -> &StackFrame {
        self.frames.last().unwrap()
    }
    
    fn frame_mut(&mut self) -> &mut StackFrame {
        self.frames.last_mut().unwrap()
    }
}