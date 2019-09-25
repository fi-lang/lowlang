pub mod memory;

use syntax::*;
use std::collections::HashMap;

#[derive(Debug)]
pub struct VM {
    memory: memory::Memory,
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
                Terminator::Abort => return None,
                Terminator::Call(f, args, Some((place, next)), Some(fail)) => {
                    let f = self.operand(f);
                    let f = self.fns.iter().nth(f as usize).unwrap().1.clone();
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
                        
                        let val = self.operand(arg.clone());
                        let bytes = val.to_le_bytes();
                        
                        for i in 0..size { self.memory.stack[loc + i] = bytes[i]; }
                    }
                    
                    self.frames.push(frame);
                    
                    let val = self.run_fn(f);
                    
                    self.frames.pop().unwrap();
                    
                    if let Some(val) = val {
                        let (loc, size) = self.place(place);
                        let bytes = val.to_le_bytes();
                        
                        for i in 0..size { self.memory.stack[loc + i] = bytes[i]; } 
                        
                        self.frame_mut().block = next;
                    } else {
                        self.frame_mut().block = fail;
                    }
                },
                Terminator::Call(f, args, Some((place, next)), None) => {
                    let f = self.operand(f);
                    let f = self.fns.iter().nth(f as usize).unwrap().1.clone();
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
                        
                        let val = self.operand(arg.clone());
                        let bytes = val.to_le_bytes();
                        
                        for i in 0..size { self.memory.stack[loc + i] = bytes[i]; }
                    }
                    
                    self.frames.push(frame);
                    
                    let val = self.run_fn(f);
                    
                    self.frames.pop().unwrap();
                    
                    if let Some(val) = val {
                        let (loc, size) = self.place(place);
                        let bytes = val.to_le_bytes();
                        
                        for i in 0..size { self.memory.stack[loc + i] = bytes[i]; } 
                        
                        self.frame_mut().block = next;
                    } else {
                        return None;
                    }
                },
                Terminator::Call(f, args, None, Some(fail)) => {
                     let f = self.operand(f);
                    let f = self.fns.iter().nth(f as usize).unwrap().1.clone();
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
                        
                        let val = self.operand(arg.clone());
                        let bytes = val.to_le_bytes();
                        
                        for i in 0..size { self.memory.stack[loc + i] = bytes[i]; }
                    }
                    
                    self.frames.push(frame);
                    
                    let val = self.run_fn(f);
                    
                    self.frames.pop().unwrap();
                    
                    if let None = val {
                        self.frame_mut().block = fail;
                    }
                },
                Terminator::Call(f, args, None, None) => {
                    let f = self.operand(f);
                    let f = self.fns.iter().nth(f as usize).unwrap().1.clone();
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
                        
                        let val = self.operand(arg.clone());
                        let bytes = val.to_le_bytes();
                        
                        for i in 0..size { self.memory.stack[loc + i] = bytes[i]; }
                    }
                    
                    self.frames.push(frame);
                    
                    let val = self.run_fn(f);
                    
                    self.frames.pop().unwrap();
                    
                    if let None = val {
                        return None;
                    } else {
                        return Some(loc);
                    }
                },
                _ => unimplemented!()
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
            RValue::Use(op) => self.operand(op),
            _ => unimplemented!()
        }
    }
    
    fn operand(&mut self, o: Operand) -> u64 {
        match o {
            Operand::Constant(c) => self.constant(c),
            Operand::Copy(p) => self.place(p).0 as u64,
            Operand::Move(p) => unimplemented!()
        }
    }
    
    fn constant(&mut self, c: Constant) -> u64 {
        match c {
            Constant::Int(v, _) => v as u64,
            Constant::UInt(v, _) => v,
            Constant::Float(v, _) => v as u64,
            Constant::Bool(b) => b as u64,
            Constant::Item(id) => {
                for (i, (name, _)) in self.fns.iter().enumerate() {
                    if name == &id.text { return i as u64; }
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