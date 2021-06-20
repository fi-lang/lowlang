use crate::*;

impl Module {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            funcs: Arena::default(),
            bodies: Arena::default(),
        }
    }

    pub fn declare_func(&mut self, name: impl Into<String>, linkage: Linkage, sig: Ty) -> FuncId {
        let idx = self.funcs.alloc(Func {
            linkage,
            sig,
            name: name.into(),
            body: None,
        });

        FuncId(idx)
    }

    pub fn define_func(&mut self, func: FuncId, body: BodyId) {
        self[func].body = Some(body);
    }

    pub fn declare_body(&mut self) -> BodyId {
        let idx = self.bodies.alloc(Body::default());

        BodyId(idx)
    }

    pub fn define_body(&mut self, body_id: BodyId) -> Builder {
        Builder {
            module: self,
            body_id,
            block_id: Block::ENTRY,
        }
    }
}

pub struct Builder<'a> {
    module: &'a mut Module,
    body_id: BodyId,
    block_id: Block,
}

pub struct SwitchBuilder<'a, 'b> {
    builder: &'b mut Builder<'a>,
    op: Var,
    cases: Vec<SwitchCase>,
}

impl<'a> Builder<'a> {
    pub fn body(&self) -> &Body {
        &self.module[self.body_id]
    }

    pub fn body_mut(&mut self) -> &mut Body {
        &mut self.module[self.body_id]
    }

    fn block(&mut self) -> &mut BlockData {
        let block_id = self.block_id;

        &mut self.body_mut()[block_id]
    }

    /// Finish the current body
    pub fn finish(self) {
    }

    /// Add a new generic parameter
    pub fn add_generic_param(&mut self, param: GenericParam) -> GenericVar {
        let i = self.body().generic_params.len();

        self.body_mut().generic_params.push(param);

        GenericVar(0, i as u8)
    }

    /// Create a new variable with type `ty`.
    pub fn create_var(&mut self, ty: Ty) -> Var {
        let idx = self.body_mut().vars.alloc(VarInfo { ty, flags: Flags::EMPTY });

        Var(idx)
    }

    /// Add a new basic block
    pub fn create_block(&mut self) -> Block {
        let idx = self.body_mut().blocks.alloc(BlockData {
            params: Vec::new(),
            instrs: Vec::new(),
            term: None,
        });

        Block(idx)
    }

    /// Set the current basic block
    pub fn set_block(&mut self, block_id: Block) {
        self.block_id = block_id;
    }

    /// Add a new basic block parameter
    pub fn add_param(&mut self, block_id: Block, ty: Ty) -> Var {
        let param = match ty.kind {
            | typ::Var(_) => {
                let var = self.create_var(ty.ptr());

                self.body_mut()[var].flags = Flags::INDIRECT;
                var
            },
            | _ => self.create_var(ty),
        };

        self.body_mut()[block_id].params.push(param);
        param
    }

    /// Set this block's terminator to unreachable
    pub fn unreachable(&mut self) {
        self.block().term = Some(Term::Unreachable);
    }

    /// Return with a set of values
    pub fn return_(&mut self, ops: impl IntoIterator<Item = Var>) {
        let mut i = 0;
        let ops = ops
            .into_iter()
            .filter_map(|op| {
                if self.body()[op].flags.is_set(Flags::INDIRECT) {
                    if self.body()[Block::ENTRY].params.len() > i && self.body()[self.body()[Block::ENTRY].params[i]].flags.is_set(Flags::RETURN) {
                        self.copy_addr(op, self.body()[Block::ENTRY].params[i], Flags::EMPTY);
                    } else {
                        let param = self.create_var(self.body().var_type(op));

                        self.body_mut()[param].flags = Flags::RETURN;
                        self.body_mut()[Block::ENTRY].params.insert(0, param);
                        self.copy_addr(op, param, Flags::EMPTY);
                    }

                    i += 1;
                    None
                } else {
                    Some(op)
                }
            })
            .collect();

        self.block().term = Some(Term::Return { ops });
    }

    /// Jumpt to the target block with a set of arguments
    pub fn br(&mut self, block: Block, args: impl IntoIterator<Item = Var>) {
        self.block().term = Some(Term::Br {
            to: BrTarget {
                block,
                args: args.into_iter().collect(),
            },
        })
    }

    /// Build a new switch terminator
    pub fn switch(&mut self, op: Var) -> SwitchBuilder<'a, '_> {
        SwitchBuilder {
            builder: self,
            op,
            cases: Vec::new(),
        }
    }

    /// Allocate space on the stack for a value of type `ty`.
    /// The returned var has type `*ty`.
    pub fn stack_alloc(&mut self, ty: Ty) -> Var {
        let ret = self.create_var(ty.clone().ptr());

        self.block().instrs.push(Instr::StackAlloc { ret, ty });

        ret
    }

    /// Deallocate a value on the stack that was previously allocated.
    /// Make sure that stack values are deallocated in reverse order from
    /// how they were allocated.
    pub fn stack_free(&mut self, addr: Var) {
        self.block().instrs.push(Instr::StackFree { addr });
    }

    /// Allocate a new box for a value of type `ty`.
    /// Boxes in this language are generational references
    /// with a single owner.
    /// The return var has type `box ty`.
    pub fn box_alloc(&mut self, ty: Ty) -> Var {
        let ret = self.create_var(ty.clone().boxed());

        self.block().instrs.push(Instr::BoxAlloc { ret, ty });

        ret
    }

    /// Deallocate a previously allocted box.
    pub fn box_free(&mut self, boxed: Var) {
        self.block().instrs.push(Instr::BoxFree { boxed });
    }

    /// Get the address of a boxed value of type `box ty`.
    /// The returned var has type `*ty`.
    pub fn box_addr(&mut self, boxed: Var) -> Var {
        if let typ::Box(ref of) = self.body().var_type(boxed).kind {
            let ret = self.create_var(of.clone().ptr());

            self.block().instrs.push(Instr::BoxAddr { ret, boxed });

            ret
        } else {
            panic!("Cannot take the address of a value that is not boxed");
        }
    }

    /// Load a value of type `*ty`.
    /// The return var has type `ty`.
    pub fn load(&mut self, addr: Var) -> Var {
        if let typ::Ptr(ref to) = self.body().var_type(addr).kind {
            let ret = self.create_var(to.clone());

            self.block().instrs.push(Instr::Load { ret, addr });

            ret
        } else {
            panic!("Cannot load a value that is not a pointer");
        }
    }

    /// Store a value of type `ty` in an addr of type `*ty`.
    pub fn store(&mut self, val: Var, addr: Var) {
        if let typ::Ptr(ref to) = self.body().var_type(addr).kind {
            if *to != self.body().var_type(val) {
                panic!("Cannot store value of type `a` in an address of type `*b`");
            }
        } else {
            panic!("Cannot store a value in an address that is not a pointer");
        }

        self.block().instrs.push(Instr::Store { val, addr });
    }

    /// Copy a value of type `ty` from one address to another.
    /// Flags:
    ///   - TAKE: the value is moved from old to new.
    ///   - INIT: the new address was previously uninitialized.
    pub fn copy_addr(&mut self, old: Var, new: Var, flags: Flags) {
        if let typ::Ptr(ref old) = self.body().var_type(old).kind {
            if let typ::Ptr(ref new) = self.body().var_type(new).kind {
                if old != new {
                    panic!("Cannot copy a value of type `a` in an address of type `*b`");
                }
            } else {
                panic!("Cannot copy a value into an address that is not a pointer");
            }
        } else {
            panic!("Cannot copy a value from an address that is not a pointer");
        }

        self.block().instrs.push(Instr::CopyAddr { old, new, flags });
    }

    /// Create a constant integer value of type `ty`.
    pub fn const_int(&mut self, val: u128, ty: Ty) -> Var {
        let ret = self.create_var(ty);

        self.block().instrs.push(Instr::ConstInt { ret, val });

        ret
    }

    /// Create a constant reference to a function.
    /// The return var will have the type of the function's signature.
    pub fn func_ref(&mut self, func: FuncId) -> Var {
        let sig = self.module[func].sig.clone();
        let ret = self.create_var(sig);

        self.block().instrs.push(Instr::FuncRef { ret, func });

        ret
    }

    /// Apply a function to some arguments.
    /// If the function is generic then the substitutions must be supplied.
    /// This function returns the same number of vars as there are returns in the signature.
    // @TODO: typecheck arguments
    pub fn apply(&mut self, func: Var, subst: impl IntoIterator<Item = Subst>, args: impl IntoIterator<Item = Var>) -> Vec<Var> {
        let mut sig = self.body().var_type(func);
        let subst = subst.into_iter().collect::<Vec<_>>();

        if let typ::Generic(_, ref ret) = sig.kind {
            sig = ret.subst(&subst, 0);
        }

        if let typ::Func(ref sig) = sig.kind {
            let mut ret_args = Vec::new();
            let mut rets = sig
                .rets
                .iter()
                .filter_map(|ret| {
                    if ret.flags.is_set(Flags::OUT) {
                        let stack_slot = self.stack_alloc(ret.ty.clone());

                        ret_args.push(stack_slot);
                        None
                    } else {
                        Some(self.create_var(ret.ty.clone()))
                    }
                })
                .collect::<Vec<_>>();

            let mut indirect_args = Vec::new();
            let mut args = ret_args
                .iter()
                .copied()
                .chain(args.into_iter().zip(&sig.params).map(|(arg, param)| {
                    if param.flags.is_set(Flags::IN) {
                        let stack_slot = self.stack_alloc(param.ty.clone());

                        self.store(arg, stack_slot);
                        indirect_args.push(stack_slot);
                        stack_slot
                    } else {
                        arg
                    }
                }))
                .collect::<Vec<_>>();

            self.block().instrs.push(Instr::Apply {
                rets: rets.clone(),
                func,
                subst,
                args,
            });

            ret_args.reverse();

            for arg in indirect_args.into_iter().rev() {
                self.stack_free(arg);
            }

            for (i, ret) in sig.rets.iter().enumerate() {
                if ret.flags.is_set(Flags::OUT) {
                    let stack_slot = ret_args.pop().unwrap();
                    let val = self.load(stack_slot);

                    self.stack_free(stack_slot);
                    rets.insert(i, val);
                }
            }

            rets
        } else {
            panic!("Cannot apply a value that is not a function");
        }
    }

    pub fn intrinsic(&mut self, name: impl AsRef<str>, args: impl IntoIterator<Item = Var>) -> Vec<Var> {
        let name = name.as_ref();

        if let Some(mut sig) = intrinsics::INTRINSICS.get(name).cloned() {
            if let typ::Generic(_, ref ret) = sig.kind {
                sig = ret.clone();
            }

            if let typ::Func(ref sig) = sig.kind {
                let rets = sig.rets.iter().map(|r| self.create_var(r.ty.clone())).collect::<Vec<_>>();

                self.block().instrs.push(Instr::Intrinsic {
                    name: name.into(),
                    rets: rets.clone(),
                    args: args.into_iter().collect(),
                });

                rets
            } else {
                unreachable!();
            }
        } else {
            panic!("unknown intrinsic {:?}", name);
        }
    }
}

impl SwitchBuilder<'_, '_> {
    pub fn case(&mut self, val: u128, block: Block, args: impl IntoIterator<Item = Var>) {
        self.cases.push(SwitchCase {
            val,
            to: BrTarget {
                block,
                args: args.into_iter().collect(),
            },
        });
    }

    pub fn build(self, block: Block, args: impl IntoIterator<Item = Var>) {
        self.builder.block().term = Some(Term::Switch {
            pred: self.op,
            cases: self.cases,
            default: BrTarget {
                block,
                args: args.into_iter().collect(),
            },
        });
    }
}