use mellis_mvir::{Module, Function, Instruction, Operand, Terminator, BasicBlock, ValueId, ValueData, LabelId};
use mellis_semantic::{SemanticContext, SemanticType, SemanticTypeId};
use std::collections::HashMap;

pub fn lower_async(module: &mut Module, ctx: &mut SemanticContext) {
    let mut async_funcs = Vec::new();
    for (i, func) in module.functions.iter().enumerate() {
        if func.is_async {
            async_funcs.push(i);
        }
    }

    let mut new_funcs = Vec::new();

    for i in async_funcs {
        let func = &module.functions[i];
        let (kickoff, resume, drop_fn) = lower_single_async_func(func, ctx);
        new_funcs.push((i, kickoff, resume, drop_fn));
    }

    for (i, kickoff, resume, drop_fn) in new_funcs.into_iter().rev() {
        module.functions[i] = kickoff;
        module.functions.push(resume);
        module.functions.push(drop_fn);
    }
}

fn lower_single_async_func(func: &Function, ctx: &mut SemanticContext) -> (Function, Function, Function) {
    // 1. Analyze Alloca instructions to form the Env Struct.
    // Env Struct layout:
    // Field 0: state (i32) -> 0 = initial, 1..K = await point k, -1 = completed
    // Field 1: poll_status (i32) -> 0 = Pending, 1 = Ready, -1 = Error/Cancelled
    // Field 2: child_future (pointer / void) -> stores active awaited future across suspension
    // Field 3..N: the original Alloca variables (local variables & parameters)
    let mut env_fields = Vec::new();
    env_fields.push(SemanticTypeId(3)); // field 0: state (i32)
    env_fields.push(SemanticTypeId(3)); // field 1: poll_status (i32)
    
    let void_ptr_ty = ctx.types.intern(SemanticType::Pointer(mellis_semantic::ty::Mutability::Mutable, SemanticTypeId(0)));
    env_fields.push(void_ptr_ty); // field 2: child_future (ptr/void)
    
    let mut alloca_map = HashMap::new(); // maps old ValueId -> env field index (starting at 3)
    for (val_id_idx, val) in func.values.iter().enumerate() {
        if let Instruction::Alloca = val.inst {
            alloca_map.insert(ValueId(val_id_idx as u32), env_fields.len() as u32);
            env_fields.push(val.ty);
        }
    }

    let env_ty_id = ctx.types.intern(SemanticType::Tuple(env_fields.clone()));
    
    // 2. Create the kickoff function
    let mut kickoff = Function {
        name: func.name.clone(),
        is_extern: false,
        is_async: false,
        ret_ty: void_ptr_ty,
        arg_count: func.arg_count,
        blocks: Vec::new(),
        values: Vec::new(),
    };
    
    let mut k_entry = BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: Vec::new(),
        terminator: None,
    };
    
    // In kickoff, allocate arguments as the first values
    let mut kickoff_arg_val_ids = Vec::new();
    for i in 0..func.arg_count {
        let arg_ty = func.values.get(i).map(|v| v.ty).unwrap_or(SemanticTypeId(0));
        let arg_val_id = ValueId(kickoff.values.len() as u32);
        kickoff.values.push(ValueData {
            inst: Instruction::Alloca,
            ty: arg_ty,
            span: None,
        });
        k_entry.insts.push(arg_val_id);
        kickoff_arg_val_ids.push(arg_val_id);
    }
    
    // env = HeapAlloc EnvStruct
    let env_val_id = ValueId(kickoff.values.len() as u32);
    kickoff.values.push(ValueData {
        inst: Instruction::HeapAlloc,
        ty: env_ty_id,
        span: None,
    });
    k_entry.insts.push(env_val_id);
    
    // env.state = 0
    let state_ptr_id = ValueId(kickoff.values.len() as u32);
    kickoff.values.push(ValueData {
        inst: Instruction::FieldPtr { base: Operand::Value(env_val_id), field_idx: 0 },
        ty: SemanticTypeId(3), // i32 ptr
        span: None,
    });
    k_entry.insts.push(state_ptr_id);
    
    let store_id = ValueId(kickoff.values.len() as u32);
    kickoff.values.push(ValueData {
        inst: Instruction::Store { ptr: Operand::Value(state_ptr_id), value: Operand::Number("0".to_string()) },
        ty: SemanticTypeId(0), // void
        span: None,
    });
    k_entry.insts.push(store_id);

    // env.poll_status = 0 (Pending)
    let status_ptr_id = ValueId(kickoff.values.len() as u32);
    kickoff.values.push(ValueData {
        inst: Instruction::FieldPtr { base: Operand::Value(env_val_id), field_idx: 1 },
        ty: SemanticTypeId(3),
        span: None,
    });
    k_entry.insts.push(status_ptr_id);

    let store_status_id = ValueId(kickoff.values.len() as u32);
    kickoff.values.push(ValueData {
        inst: Instruction::Store { ptr: Operand::Value(status_ptr_id), value: Operand::Number("0".to_string()) },
        ty: SemanticTypeId(0),
        span: None,
    });
    k_entry.insts.push(store_status_id);

    // env.child_future = null
    let child_ptr_id = ValueId(kickoff.values.len() as u32);
    kickoff.values.push(ValueData {
        inst: Instruction::FieldPtr { base: Operand::Value(env_val_id), field_idx: 2 },
        ty: void_ptr_ty,
        span: None,
    });
    k_entry.insts.push(child_ptr_id);

    let null_id = ValueId(kickoff.values.len() as u32);
    kickoff.values.push(ValueData {
        inst: Instruction::Null { ty: void_ptr_ty },
        ty: void_ptr_ty,
        span: None,
    });
    k_entry.insts.push(null_id);

    let store_child_id = ValueId(kickoff.values.len() as u32);
    kickoff.values.push(ValueData {
        inst: Instruction::Store { ptr: Operand::Value(child_ptr_id), value: Operand::Value(null_id) },
        ty: SemanticTypeId(0),
        span: None,
    });
    k_entry.insts.push(store_child_id);
    
    // Copy incoming parameters into env fields (fields 3..)
    for (i, &arg_val_id) in kickoff_arg_val_ids.iter().enumerate() {
        let old_alloca_id = ValueId(i as u32);
        if let Some(&field_idx) = alloca_map.get(&old_alloca_id) {
            let field_ty = env_fields.get(field_idx as usize).copied().unwrap_or(SemanticTypeId(0));
            
            let field_ptr_id = ValueId(kickoff.values.len() as u32);
            kickoff.values.push(ValueData {
                inst: Instruction::FieldPtr { base: Operand::Value(env_val_id), field_idx },
                ty: field_ty,
                span: None,
            });
            k_entry.insts.push(field_ptr_id);
            
            let load_arg_id = ValueId(kickoff.values.len() as u32);
            kickoff.values.push(ValueData {
                inst: Instruction::Load { ptr: Operand::Value(arg_val_id) },
                ty: field_ty,
                span: None,
            });
            k_entry.insts.push(load_arg_id);
            
            let store_arg_id = ValueId(kickoff.values.len() as u32);
            kickoff.values.push(ValueData {
                inst: Instruction::Store { ptr: Operand::Value(field_ptr_id), value: Operand::Value(load_arg_id) },
                ty: SemanticTypeId(0),
                span: None,
            });
            k_entry.insts.push(store_arg_id);
        }
    }
    
    k_entry.terminator = Some(Terminator::Ret { value: Some(Operand::Value(env_val_id)) });
    kickoff.blocks.push(k_entry);

    // 3. Create the resume function
    let mut resume_name = func.name.clone();
    resume_name.name = format!("{}_resume", resume_name.name);
    
    let mut resume = Function {
        name: resume_name,
        is_extern: false,
        is_async: false,
        ret_ty: func.ret_ty,
        arg_count: 1, // takes only the Env Struct pointer
        blocks: Vec::new(),
        values: Vec::new(),
    };
    
    // First value in resume is the env pointer alloca
    let env_arg_id = ValueId(resume.values.len() as u32);
    resume.values.push(ValueData {
        inst: Instruction::Alloca,
        ty: env_ty_id,
        span: None,
    });
    
    let mut val_map = HashMap::new();
    
    // Pre-create FieldPtr values for all Allocas
    for (i, val) in func.values.iter().enumerate() {
        let old_id = ValueId(i as u32);
        if let Instruction::Alloca = val.inst {
            if let Some(&field_idx) = alloca_map.get(&old_id) {
                let new_id = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::FieldPtr {
                        base: Operand::Value(env_arg_id),
                        field_idx,
                    },
                    ty: val.ty,
                    span: val.span.clone(),
                });
                val_map.insert(old_id, new_id);
            }
        }
    }
    
    // Helper to map operands
    let map_op = |op: &Operand, v_map: &HashMap<ValueId, ValueId>| -> Operand {
        match op {
            Operand::Value(vid) => Operand::Value(*v_map.get(vid).unwrap_or(vid)),
            _ => op.clone(),
        }
    };
    
    // Count and track await points for block splitting
    let mut await_points: Vec<(usize, LabelId, ValueId)> = Vec::new(); // (state_id, resume_label, old_vid)
    let mut total_awaits = 0;
    
    let mut translated_blocks = Vec::new();
    
    for block in &func.blocks {
        let mut curr_label = block.label.clone();
        let mut curr_insts = Vec::new();
        
        for &old_vid in &block.insts {
            let old_val = &func.values[old_vid.0 as usize];
            if let Instruction::Alloca = old_val.inst {
                // Alloca is already mapped to FieldPtr and pushed into entry_dispatch
                continue;
            }
            
            if let Instruction::Await { future } = &old_val.inst {
                total_awaits += 1;
                let state_id = total_awaits;
                let resume_label = LabelId { name: format!("resume_state_{}", state_id) };
                await_points.push((state_id, resume_label.clone(), old_vid));
                
                // 1. Store active child future into env.child_future (field 2)
                let mapped_fut = map_op(future, &val_map);
                let child_ptr_id = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::FieldPtr { base: Operand::Value(env_arg_id), field_idx: 2 },
                    ty: SemanticTypeId(0),
                    span: None,
                });
                curr_insts.push(child_ptr_id);

                let store_child_id = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::Store {
                        ptr: Operand::Value(child_ptr_id),
                        value: mapped_fut.clone(),
                    },
                    ty: SemanticTypeId(0),
                    span: None,
                });
                curr_insts.push(store_child_id);

                // 2. Store next state into env.state (field 0)
                let state_ptr_id = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::FieldPtr { base: Operand::Value(env_arg_id), field_idx: 0 },
                    ty: SemanticTypeId(3),
                    span: None,
                });
                curr_insts.push(state_ptr_id);
                
                let store_state_id = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::Store {
                        ptr: Operand::Value(state_ptr_id),
                        value: Operand::Number(state_id.to_string()),
                    },
                    ty: SemanticTypeId(0),
                    span: None,
                });
                curr_insts.push(store_state_id);

                // 3. Store poll_status = 0 (Pending) into env.poll_status (field 1)
                let status_ptr_id = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::FieldPtr { base: Operand::Value(env_arg_id), field_idx: 1 },
                    ty: SemanticTypeId(3),
                    span: None,
                });
                curr_insts.push(status_ptr_id);

                let store_status_id = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::Store {
                        ptr: Operand::Value(status_ptr_id),
                        value: Operand::Number("0".to_string()),
                    },
                    ty: SemanticTypeId(0),
                    span: None,
                });
                curr_insts.push(store_status_id);
                
                // 4. Construct Poll::Pending ( { 0, undef } ) and return it
                let pending_ret_alloc = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::Alloca,
                    ty: resume.ret_ty,
                    span: None,
                });
                curr_insts.push(pending_ret_alloc);

                let pending_ret_status_ptr = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::FieldPtr { base: Operand::Value(pending_ret_alloc), field_idx: 0 },
                    ty: SemanticTypeId(3),
                    span: None,
                });
                curr_insts.push(pending_ret_status_ptr);

                let pending_ret_store = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::Store { ptr: Operand::Value(pending_ret_status_ptr), value: Operand::Number("0".to_string()) },
                    ty: SemanticTypeId(0),
                    span: None,
                });
                curr_insts.push(pending_ret_store);

                let pending_ret_val = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::Load { ptr: Operand::Value(pending_ret_alloc) },
                    ty: resume.ret_ty,
                    span: None,
                });
                curr_insts.push(pending_ret_val);

                // Suspend and terminate current block
                let suspended_block = BasicBlock {
                    label: curr_label,
                    insts: curr_insts,
                    terminator: Some(Terminator::Ret { value: Some(Operand::Value(pending_ret_val)) }),
                };
                translated_blocks.push(suspended_block);
                
                // Start next block
                curr_label = resume_label;
                curr_insts = Vec::new();
                
                // Load child future from env
                let child_fut_field_id = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::FieldPtr { base: Operand::Value(env_arg_id), field_idx: 2 },
                    ty: env_ty_id,
                    span: None,
                });
                curr_insts.push(child_fut_field_id);
                
                let loaded_child_fut_id = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::Load { ptr: Operand::Value(child_fut_field_id) },
                    ty: void_ptr_ty,
                    span: None,
                });
                curr_insts.push(loaded_child_fut_id);
                
                // The result of await: create mapped value
                let await_res_id = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::Await { future: Operand::Value(loaded_child_fut_id) },
                    ty: old_val.ty,
                    span: old_val.span.clone(),
                });
                val_map.insert(old_vid, await_res_id);
                curr_insts.push(await_res_id);

                // Nullify child_future in EnvStruct after Await completes
                let null_val_id = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::Null { ty: void_ptr_ty },
                    ty: void_ptr_ty,
                    span: None,
                });
                curr_insts.push(null_val_id);

                let store_null_id = ValueId(resume.values.len() as u32);
                resume.values.push(ValueData {
                    inst: Instruction::Store {
                        ptr: Operand::Value(child_fut_field_id),
                        value: Operand::Value(null_val_id),
                    },
                    ty: SemanticTypeId(0),
                    span: None,
                });
                curr_insts.push(store_null_id);

                continue;
            }
            
            // General instruction mapping
            let mut new_inst = old_val.inst.clone();
            match &mut new_inst {
                Instruction::Assign(op) => *op = map_op(op, &val_map),
                Instruction::Store { ptr, value } => {
                    *ptr = map_op(ptr, &val_map);
                    *value = map_op(value, &val_map);
                }
                Instruction::Load { ptr } => *ptr = map_op(ptr, &val_map),
                Instruction::Add { left, right } | Instruction::Sub { left, right } | Instruction::Mul { left, right } |
                Instruction::Div { left, right } | Instruction::Rem { left, right } |
                Instruction::Eq { left, right } | Instruction::NotEq { left, right } |
                Instruction::LessThan { left, right } | Instruction::LessOrEq { left, right } |
                Instruction::GreaterThan { left, right } | Instruction::GreaterOrEq { left, right } |
                Instruction::BitAnd { left, right } | Instruction::BitOr { left, right } |
                Instruction::BitXor { left, right } | Instruction::Shl { left, right } |
                Instruction::Shr { left, right } => {
                    *left = map_op(left, &val_map);
                    *right = map_op(right, &val_map);
                }
                Instruction::CallDirect { args, .. } => {
                    for arg in args { *arg = map_op(arg, &val_map); }
                }
                Instruction::CallIndirect { callee, args, .. } => {
                    *callee = map_op(callee, &val_map);
                    for arg in args { *arg = map_op(arg, &val_map); }
                }
                Instruction::CallClosure { closure, args, .. } => {
                    *closure = map_op(closure, &val_map);
                    for arg in args { *arg = map_op(arg, &val_map); }
                }
                Instruction::MakeClosure { env_ptr, captures, .. } => {
                    *env_ptr = map_op(env_ptr, &val_map);
                    for cap in captures {
                        cap.source = *val_map.get(&cap.source).unwrap_or(&cap.source);
                    }
                }
                Instruction::CallVirt { obj, args, .. } => {
                    *obj = map_op(obj, &val_map);
                    for arg in args { *arg = map_op(arg, &val_map); }
                }
                Instruction::MakeTraitObject { data_ptr, .. } => {
                    *data_ptr = map_op(data_ptr, &val_map);
                }
                Instruction::Borrow { base, .. } => *base = map_op(base, &val_map),
                Instruction::Variant { args, .. } => {
                    for arg in args { *arg = map_op(arg, &val_map); }
                }
                Instruction::Tag { value } | Instruction::Extract { value, .. } | Instruction::FieldPtr { base: value, .. } |
                Instruction::Drop { value, .. } | Instruction::BoxNew { value } | Instruction::BoxFree { value } |
                Instruction::MarkInit { value } | Instruction::PtrCast { ptr: value, .. } | Instruction::Await { future: value } => {
                    *value = map_op(value, &val_map);
                }
                Instruction::PtrOffset { ptr, offset } => {
                    *ptr = map_op(ptr, &val_map);
                    *offset = map_op(offset, &val_map);
                }
                Instruction::BoundsCheck { index, len } => {
                    *index = map_op(index, &val_map);
                    *len = map_op(len, &val_map);
                }
                Instruction::Alloca | Instruction::HeapAlloc | Instruction::Null { .. } |
                Instruction::SizeOf { .. } | Instruction::AlignOf { .. } => {}
            }
            
            let new_id = ValueId(resume.values.len() as u32);
            resume.values.push(ValueData {
                inst: new_inst,
                ty: old_val.ty,
                span: old_val.span.clone(),
            });
            val_map.insert(old_vid, new_id);
            curr_insts.push(new_id);
        }
        
        let mut term = block.terminator.clone();
        if let Some(t) = &mut term {
            match t {
                Terminator::CondBr { condition, .. } => {
                    *condition = map_op(condition, &val_map);
                }
                Terminator::Ret { value } => {
                    if let Some(val) = value {
                        *val = map_op(val, &val_map);
                    }
                    // Before Ret, mark state = 9999 (completed) and poll_status = 1 (Ready)
                    let state_ptr_id = ValueId(resume.values.len() as u32);
                    resume.values.push(ValueData {
                        inst: Instruction::FieldPtr { base: Operand::Value(env_arg_id), field_idx: 0 },
                        ty: SemanticTypeId(3),
                        span: None,
                    });
                    curr_insts.push(state_ptr_id);
                    
                    let store_state_id = ValueId(resume.values.len() as u32);
                    resume.values.push(ValueData {
                        inst: Instruction::Store {
                            ptr: Operand::Value(state_ptr_id),
                            value: Operand::Number("9999".to_string()),
                        },
                        ty: SemanticTypeId(0),
                        span: None,
                    });
                    curr_insts.push(store_state_id);

                    let status_ptr_id = ValueId(resume.values.len() as u32);
                    resume.values.push(ValueData {
                        inst: Instruction::FieldPtr { base: Operand::Value(env_arg_id), field_idx: 1 },
                        ty: SemanticTypeId(3),
                        span: None,
                    });
                    curr_insts.push(status_ptr_id);

                    let store_status_id = ValueId(resume.values.len() as u32);
                    resume.values.push(ValueData {
                        inst: Instruction::Store {
                            ptr: Operand::Value(status_ptr_id),
                            value: Operand::Number("1".to_string()),
                        },
                        ty: SemanticTypeId(0),
                        span: None,
                    });
                    curr_insts.push(store_status_id);

                    // Construct Poll::Ready ( { 1, val } )
                    let ready_ret_alloc = ValueId(resume.values.len() as u32);
                    resume.values.push(ValueData {
                        inst: Instruction::Alloca,
                        ty: resume.ret_ty,
                        span: None,
                    });
                    curr_insts.push(ready_ret_alloc);

                    let ready_ret_status_ptr = ValueId(resume.values.len() as u32);
                    resume.values.push(ValueData {
                        inst: Instruction::FieldPtr { base: Operand::Value(ready_ret_alloc), field_idx: 0 },
                        ty: SemanticTypeId(3),
                        span: None,
                    });
                    curr_insts.push(ready_ret_status_ptr);

                    let ready_ret_status_store = ValueId(resume.values.len() as u32);
                    resume.values.push(ValueData {
                        inst: Instruction::Store { ptr: Operand::Value(ready_ret_status_ptr), value: Operand::Number("1".to_string()) },
                        ty: SemanticTypeId(0),
                        span: None,
                    });
                    curr_insts.push(ready_ret_status_store);

                    if let Some(val) = value {
                        let ready_ret_val_ptr = ValueId(resume.values.len() as u32);
                        resume.values.push(ValueData {
                            inst: Instruction::FieldPtr { base: Operand::Value(ready_ret_alloc), field_idx: 1 },
                            ty: SemanticTypeId(0), // Doesn't matter
                            span: None,
                        });
                        curr_insts.push(ready_ret_val_ptr);

                        let ready_ret_val_store = ValueId(resume.values.len() as u32);
                        resume.values.push(ValueData {
                            inst: Instruction::Store { ptr: Operand::Value(ready_ret_val_ptr), value: val.clone() },
                            ty: SemanticTypeId(0),
                            span: None,
                        });
                        curr_insts.push(ready_ret_val_store);
                    }

                    let ready_ret_val_final = ValueId(resume.values.len() as u32);
                    resume.values.push(ValueData {
                        inst: Instruction::Load { ptr: Operand::Value(ready_ret_alloc) },
                        ty: resume.ret_ty,
                        span: None,
                    });
                    curr_insts.push(ready_ret_val_final);

                    *value = Some(Operand::Value(ready_ret_val_final));
                }
                _ => {}
            }
        }
        
        translated_blocks.push(BasicBlock {
            label: curr_label,
            insts: curr_insts,
            terminator: term,
        });
    }
    
    // Generate Entry Dispatcher
    let initial_entry_label = func.blocks.first().map(|b| b.label.clone()).unwrap_or(LabelId { name: "entry".to_string() });
    
    if await_points.is_empty() {
        // No await points: single direct branch to entry
        let mut dispatch_block = BasicBlock {
            label: LabelId { name: "entry_dispatch".to_string() },
            insts: vec![],
            terminator: Some(Terminator::Br { target: initial_entry_label }),
        };
        // Prepend all pre-created FieldPtrs for allocas into dispatch_block
        let mut alloca_field_ptrs = Vec::new();
        for (&old_id, &new_id) in &val_map {
            if let Instruction::Alloca = func.values[old_id.0 as usize].inst {
                alloca_field_ptrs.push(new_id);
            }
        }
        // Sort to maintain deterministic order
        alloca_field_ptrs.sort_by_key(|id| id.0);
        
        // Insert them right after the env_arg_id
        dispatch_block.insts.splice(0..0, alloca_field_ptrs);
        dispatch_block.insts.insert(0, env_arg_id);
        
        resume.blocks.push(dispatch_block);
        resume.blocks.extend(translated_blocks);
    } else {
        // Build dispatch chain
        let state_ptr_id = ValueId(resume.values.len() as u32);
        resume.values.push(ValueData {
            inst: Instruction::FieldPtr { base: Operand::Value(env_arg_id), field_idx: 0 },
            ty: SemanticTypeId(3),
            span: None,
        });
        
        let state_val_id = ValueId(resume.values.len() as u32);
        resume.values.push(ValueData {
            inst: Instruction::Load { ptr: Operand::Value(state_ptr_id) },
            ty: SemanticTypeId(3),
            span: None,
        });
        
        let mut dispatch_blocks = Vec::new();
        
        // Dispatch 0 block
        let mut entry_dispatch = BasicBlock {
            label: LabelId { name: "entry_dispatch".to_string() },
            insts: vec![env_arg_id],
            terminator: None,
        };
        
        let mut alloca_field_ptrs = Vec::new();
        for (&old_id, &new_id) in &val_map {
            if let Instruction::Alloca = func.values[old_id.0 as usize].inst {
                alloca_field_ptrs.push(new_id);
            }
        }
        alloca_field_ptrs.sort_by_key(|id| id.0);
        entry_dispatch.insts.extend(alloca_field_ptrs);
        entry_dispatch.insts.extend(vec![state_ptr_id, state_val_id]);

        
        let eq_0_id = ValueId(resume.values.len() as u32);
        resume.values.push(ValueData {
            inst: Instruction::Eq {
                left: Operand::Value(state_val_id),
                right: Operand::Number("0".to_string()),
            },
            ty: SemanticTypeId(1), // bool
            span: None,
        });
        entry_dispatch.insts.push(eq_0_id);
        
        let next_dispatch_label = LabelId { name: "dispatch_state_1".to_string() };
        entry_dispatch.terminator = Some(Terminator::CondBr {
            condition: Operand::Value(eq_0_id),
            true_target: initial_entry_label,
            false_target: next_dispatch_label,
        });
        dispatch_blocks.push(entry_dispatch);
        
        // Intermediate dispatch blocks for state 1..K
        for (idx, (state_id, resume_label, _)) in await_points.iter().enumerate() {
            let curr_d_label = LabelId { name: format!("dispatch_state_{}", state_id) };
            let mut d_block = BasicBlock {
                label: curr_d_label,
                insts: Vec::new(),
                terminator: None,
            };
            
            let eq_k_id = ValueId(resume.values.len() as u32);
            resume.values.push(ValueData {
                inst: Instruction::Eq {
                    left: Operand::Value(state_val_id),
                    right: Operand::Number(state_id.to_string()),
                },
                ty: SemanticTypeId(1), // bool
                span: None,
            });
            d_block.insts.push(eq_k_id);
            
            let false_target = if idx + 1 < await_points.len() {
                LabelId { name: format!("dispatch_state_{}", await_points[idx + 1].0) }
            } else {
                LabelId { name: "unreachable_dispatch".to_string() }
            };
            
            d_block.terminator = Some(Terminator::CondBr {
                condition: Operand::Value(eq_k_id),
                true_target: resume_label.clone(),
                false_target,
            });
            dispatch_blocks.push(d_block);
        }
        
        // Unreachable fallback block (handles completed state == -1 or invalid states)
        dispatch_blocks.push(BasicBlock {
            label: LabelId { name: "unreachable_dispatch".to_string() },
            insts: Vec::new(),
            terminator: Some(Terminator::Unreachable),
        });
        
        resume.blocks.extend(dispatch_blocks);
        resume.blocks.extend(translated_blocks);
    }

    // 4. Create the drop/cancellation cleanup function
    let mut drop_name = func.name.clone();
    drop_name.name = format!("{}_drop", drop_name.name);

    let mut drop_fn = Function {
        name: drop_name,
        is_extern: false,
        is_async: false,
        ret_ty: SemanticTypeId(0), // void
        arg_count: 1, // takes only the Env Struct pointer
        blocks: Vec::new(),
        values: Vec::new(),
    };

    let drop_env_arg_id = ValueId(drop_fn.values.len() as u32);
    drop_fn.values.push(ValueData {
        inst: Instruction::Alloca,
        ty: env_ty_id,
        span: None,
    });

    let drop_state_ptr_id = ValueId(drop_fn.values.len() as u32);
    drop_fn.values.push(ValueData {
        inst: Instruction::FieldPtr { base: Operand::Value(drop_env_arg_id), field_idx: 0 },
        ty: SemanticTypeId(3),
        span: None,
    });

    let drop_state_val_id = ValueId(drop_fn.values.len() as u32);
    drop_fn.values.push(ValueData {
        inst: Instruction::Load { ptr: Operand::Value(drop_state_ptr_id) },
        ty: SemanticTypeId(3),
        span: None,
    });

    let mut drop_entry = BasicBlock {
        label: LabelId { name: "entry".to_string() },
        insts: vec![drop_env_arg_id, drop_state_ptr_id, drop_state_val_id],
        terminator: None,
    };

    let free_env_label = LabelId { name: "free_env".to_string() };

    // 1. Check if state == 9999 (Completed -> jump directly to free_env)
    let is_completed_id = ValueId(drop_fn.values.len() as u32);
    drop_fn.values.push(ValueData {
        inst: Instruction::Eq {
            left: Operand::Value(drop_state_val_id),
            right: Operand::Number("9999".to_string()),
        },
        ty: SemanticTypeId(1),
        span: None,
    });
    drop_entry.insts.push(is_completed_id);

    let free_env_label = LabelId { name: "free_env".to_string() };
    let dispatch_state_0_label = LabelId { name: "drop_dispatch_0".to_string() };

    drop_entry.terminator = Some(Terminator::CondBr {
        condition: Operand::Value(is_completed_id),
        true_target: free_env_label.clone(),
        false_target: dispatch_state_0_label.clone(),
    });
    drop_fn.blocks.push(drop_entry);

    // Compute static suspension state map from borrowck
    let async_state_map = mellis_borrowck::suspension::compute_suspension_states(func, Some(ctx));
    let num_states = await_points.len();

    // Generate dispatch blocks and cleanup blocks for state 0..=num_states
    for state_idx in 0..=num_states {
        let curr_dispatch_label = LabelId { name: format!("drop_dispatch_{}", state_idx) };
        let next_dispatch_label = if state_idx < num_states {
            LabelId { name: format!("drop_dispatch_{}", state_idx + 1) }
        } else {
            free_env_label.clone()
        };

        let cleanup_label = LabelId { name: format!("drop_state_{}", state_idx) };

        let is_state_k_id = ValueId(drop_fn.values.len() as u32);
        drop_fn.values.push(ValueData {
            inst: Instruction::Eq {
                left: Operand::Value(drop_state_val_id),
                right: Operand::Number(state_idx.to_string()),
            },
            ty: SemanticTypeId(1),
            span: None,
        });

        let dispatch_block = BasicBlock {
            label: curr_dispatch_label,
            insts: vec![is_state_k_id],
            terminator: Some(Terminator::CondBr {
                condition: Operand::Value(is_state_k_id),
                true_target: cleanup_label.clone(),
                false_target: next_dispatch_label,
            }),
        };
        drop_fn.blocks.push(dispatch_block);

        // Build cleanup block for state_idx
        let mut cleanup_block = BasicBlock {
            label: cleanup_label,
            insts: Vec::new(),
            terminator: Some(Terminator::Br { target: free_env_label.clone() }),
        };

        // 1. If state_idx > 0, drop active child future if any
        if state_idx > 0 {
            let child_future_field_ptr = ValueId(drop_fn.values.len() as u32);
            drop_fn.values.push(ValueData {
                inst: Instruction::FieldPtr { base: Operand::Value(drop_env_arg_id), field_idx: 2 },
                ty: env_ty_id,
                span: None,
            });
            cleanup_block.insts.push(child_future_field_ptr);

            let load_child_fut = ValueId(drop_fn.values.len() as u32);
            drop_fn.values.push(ValueData {
                inst: Instruction::Load { ptr: Operand::Value(child_future_field_ptr) },
                ty: void_ptr_ty,
                span: None,
            });
            cleanup_block.insts.push(load_child_fut);

            let drop_child_fut = ValueId(drop_fn.values.len() as u32);
            drop_fn.values.push(ValueData {
                inst: Instruction::Drop { value: Operand::Value(load_child_fut), ty: void_ptr_ty, callee: None },
                ty: SemanticTypeId(0),
                span: None,
            });
            cleanup_block.insts.push(drop_child_fut);
        }

        // 2. Drop live places for state_idx
        let suspension_state = if state_idx == 0 {
            &async_state_map.initial_state
        } else {
            let (_, _, await_vid) = await_points[state_idx - 1];
            async_state_map.await_states.get(&await_vid).unwrap_or(&async_state_map.initial_state)
        };

        for place in &suspension_state.live_places {
            if let Some(&env_field_idx) = alloca_map.get(&place.local) {
                let env_field_ty = env_fields[env_field_idx as usize];
                emit_drop_for_place(
                    &mut drop_fn,
                    &mut cleanup_block,
                    drop_env_arg_id,
                    env_field_idx as usize,
                    env_field_ty,
                    place,
                    ctx,
                );
            }
        }

        drop_fn.blocks.push(cleanup_block);
    }

    // free_env block: BoxFree(env) and Ret None
    let mut free_block = BasicBlock {
        label: free_env_label,
        insts: Vec::new(),
        terminator: Some(Terminator::Ret { value: None }),
    };

    let free_val_id = ValueId(drop_fn.values.len() as u32);
    drop_fn.values.push(ValueData {
        inst: Instruction::BoxFree { value: Operand::Value(drop_env_arg_id) },
        ty: SemanticTypeId(0),
        span: None,
    });
    free_block.insts.push(free_val_id);
    drop_fn.blocks.push(free_block);
    
    (kickoff, resume, drop_fn)
}

fn resolve_place_type(
    ctx: &SemanticContext,
    mut current_ty: SemanticTypeId,
    projections: &[mellis_borrowck::place::Projection],
) -> Option<SemanticTypeId> {
    for proj in projections {
        match proj {
            mellis_borrowck::place::Projection::Field(idx) => {
                match ctx.types.get(current_ty) {
                    SemanticType::Struct(_, fields) | SemanticType::Tuple(fields) => {
                        current_ty = *fields.get(*idx)?;
                    }
                    _ => return None,
                }
            }
            mellis_borrowck::place::Projection::Deref => {
                match ctx.types.get(current_ty) {
                    SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => {
                        current_ty = *inner;
                    }
                    _ => return None,
                }
            }
            mellis_borrowck::place::Projection::Index => {
                match ctx.types.get(current_ty) {
                    SemanticType::Array(inner, _) | SemanticType::Slice(inner) => {
                        current_ty = *inner;
                    }
                    _ => return None,
                }
            }
        }
    }
    Some(current_ty)
}

fn emit_drop_for_place(
    drop_fn: &mut Function,
    block: &mut BasicBlock,
    drop_env_arg_id: ValueId,
    env_field_idx: usize,
    env_field_ty: SemanticTypeId,
    place: &mellis_borrowck::place::Place,
    ctx: &SemanticContext,
) {
    if let Some(target_ty) = resolve_place_type(ctx, env_field_ty, &place.projections) {
        if !ctx.needs_drop(target_ty) {
            return;
        }

        // 1. Get base field ptr in EnvStruct
        let mut curr_ptr = ValueId(drop_fn.values.len() as u32);
        drop_fn.values.push(ValueData {
            inst: Instruction::FieldPtr {
                base: Operand::Value(drop_env_arg_id),
                field_idx: env_field_idx as u32,
            },
            ty: env_field_ty,
            span: None,
        });
        block.insts.push(curr_ptr);

        let mut curr_ty = env_field_ty;

        // 2. Walk projections
        for proj in &place.projections {
            match proj {
                mellis_borrowck::place::Projection::Field(idx) => {
                    let next_field_ty = match ctx.types.get(curr_ty) {
                        SemanticType::Struct(_, fields) | SemanticType::Tuple(fields) => {
                            if let Some(&f_ty) = fields.get(*idx) {
                                f_ty
                            } else {
                                return;
                            }
                        }
                        _ => return,
                    };
                    let next_ptr = ValueId(drop_fn.values.len() as u32);
                    drop_fn.values.push(ValueData {
                        inst: Instruction::FieldPtr {
                            base: Operand::Value(curr_ptr),
                            field_idx: *idx as u32,
                        },
                        ty: curr_ty,
                        span: None,
                    });
                    block.insts.push(next_ptr);
                    curr_ptr = next_ptr;
                    curr_ty = next_field_ty;
                }
                mellis_borrowck::place::Projection::Deref => {
                    let inner_ty = match ctx.types.get(curr_ty) {
                        SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => *inner,
                        _ => return,
                    };
                    let deref_ptr = ValueId(drop_fn.values.len() as u32);
                    drop_fn.values.push(ValueData {
                        inst: Instruction::Load {
                            ptr: Operand::Value(curr_ptr),
                        },
                        ty: inner_ty,
                        span: None,
                    });
                    block.insts.push(deref_ptr);
                    curr_ptr = deref_ptr;
                    curr_ty = inner_ty;
                }
                _ => return,
            }
        }

        // 3. Load the value at curr_ptr and drop it
        let load_id = ValueId(drop_fn.values.len() as u32);
        drop_fn.values.push(ValueData {
            inst: Instruction::Load {
                ptr: Operand::Value(curr_ptr),
            },
            ty: target_ty,
            span: None,
        });
        block.insts.push(load_id);

        let drop_id = ValueId(drop_fn.values.len() as u32);
        drop_fn.values.push(ValueData {
            inst: Instruction::Drop {
                value: Operand::Value(load_id),
                ty: target_ty,
                callee: None,
            },
            ty: SemanticTypeId(0),
            span: None,
        });
        block.insts.push(drop_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mellis_mvir::*;
    use mellis_semantic::SemanticContext;

    #[test]
    fn test_async_lowering_no_await() {
        let mut ctx = SemanticContext::new();
        let mut module = Module::new();
        
        let func = Function {
            name: GlobalId { name: "simple_async".to_string(), symbol_id: None },
            is_extern: false,
            is_async: true,
            arg_count: 0,
            ret_ty: SemanticTypeId(3),
            blocks: vec![
                BasicBlock {
                    label: LabelId { name: "entry".to_string() },
                    insts: vec![ValueId(0), ValueId(1)],
                    terminator: Some(Terminator::Ret { value: Some(Operand::Value(ValueId(1))) }),
                }
            ],
            values: vec![
                ValueData { inst: Instruction::Alloca, ty: SemanticTypeId(3), span: None },
                ValueData { inst: Instruction::Load { ptr: Operand::Value(ValueId(0)) }, ty: SemanticTypeId(3), span: None },
            ],
        };
        
        module.functions.push(func);
        lower_async(&mut module, &mut ctx);
        
        assert_eq!(module.functions.len(), 3);
        let kickoff = &module.functions[0];
        let resume = &module.functions[1];
        let drop_fn = &module.functions[2];
        
        assert_eq!(kickoff.name.name, "simple_async");
        assert_eq!(resume.name.name, "simple_async_resume");
        assert_eq!(drop_fn.name.name, "simple_async_drop");
        
        // Verify kickoff allocates env and returns it
        assert_eq!(kickoff.blocks.len(), 1);
        assert!(matches!(kickoff.values[0].inst, Instruction::HeapAlloc));
        
        // Verify resume entry dispatch
        assert_eq!(resume.blocks[0].label.name, "entry_dispatch");
        assert!(matches!(resume.blocks[0].terminator, Some(Terminator::Br { .. })));
        
        // Verifier must pass
        assert!(mellis_optimizer::verify_module(&module).is_ok());
    }

    #[test]
    fn test_async_lowering_multiple_awaits() {
        let mut ctx = SemanticContext::new();
        let mut module = Module::new();
        
        let func = Function {
            name: GlobalId { name: "multi_await".to_string(), symbol_id: None },
            is_extern: false,
            is_async: true,
            arg_count: 0,
            ret_ty: SemanticTypeId(3),
            blocks: vec![
                BasicBlock {
                    label: LabelId { name: "entry".to_string() },
                    insts: vec![ValueId(0), ValueId(1), ValueId(2)],
                    terminator: Some(Terminator::Ret { value: Some(Operand::Value(ValueId(2))) }),
                }
            ],
            values: vec![
                ValueData { inst: Instruction::Await { future: Operand::Number("1".to_string()) }, ty: SemanticTypeId(3), span: None },
                ValueData { inst: Instruction::Await { future: Operand::Number("2".to_string()) }, ty: SemanticTypeId(3), span: None },
                ValueData { inst: Instruction::Add { left: Operand::Value(ValueId(0)), right: Operand::Value(ValueId(1)) }, ty: SemanticTypeId(3), span: None },
            ],
        };
        
        module.functions.push(func);
        lower_async(&mut module, &mut ctx);
        
        assert_eq!(module.functions.len(), 3);
        let resume = &module.functions[1];
        let drop_fn = &module.functions[2];
        
        // Check that dispatch blocks and split resume blocks exist
        let block_labels: Vec<&str> = resume.blocks.iter().map(|b| b.label.name.as_str()).collect();
        assert!(block_labels.contains(&"entry_dispatch"));
        assert!(block_labels.contains(&"dispatch_state_1"));
        assert!(block_labels.contains(&"dispatch_state_2"));
        assert!(block_labels.contains(&"resume_state_1"));
        assert!(block_labels.contains(&"resume_state_2"));
        assert!(block_labels.contains(&"unreachable_dispatch"));

        // Check that drop function contains state inspection and box free
        let drop_labels: Vec<&str> = drop_fn.blocks.iter().map(|b| b.label.name.as_str()).collect();
        assert!(drop_labels.contains(&"entry"));
        assert!(drop_labels.contains(&"drop_state_1"));
        assert!(drop_labels.contains(&"drop_state_2"));
        assert!(drop_labels.contains(&"free_env"));
        
        // Verifier must pass
        assert!(mellis_optimizer::verify_module(&module).is_ok());
    }

    #[test]
    fn test_async_cancellation_drop_emits_child_and_local_drops() {
        let mut ctx = SemanticContext::new();
        let mut module = Module::new();

        // Create a Boxed heap resource type (which needs Drop)
        let struct_ty = ctx.types.intern(SemanticType::Box(SemanticTypeId(3)));

        let func = Function {
            name: GlobalId { name: "cancellation_test".to_string(), symbol_id: None },
            is_extern: false,
            is_async: true,
            arg_count: 0,
            ret_ty: SemanticTypeId(3),
            blocks: vec![
                BasicBlock {
                    label: LabelId { name: "entry".to_string() },
                    insts: vec![ValueId(0), ValueId(1), ValueId(2)],
                    terminator: Some(Terminator::Ret { value: Some(Operand::Number("0".to_string())) }),
                }
            ],
            values: vec![
                ValueData { inst: Instruction::Alloca, ty: struct_ty, span: None },
                ValueData { inst: Instruction::MarkInit { value: Operand::Value(ValueId(0)) }, ty: SemanticTypeId(0), span: None },
                ValueData { inst: Instruction::Await { future: Operand::Number("42".to_string()) }, ty: SemanticTypeId(3), span: None },
            ],
        };

        module.functions.push(func);
        lower_async(&mut module, &mut ctx);

        assert_eq!(module.functions.len(), 3);
        let drop_fn = &module.functions[2];

        // Verify drop_fn contains Drop for child future and Drop for the local struct
        let drop_inst_count = drop_fn.values.iter().filter(|v| matches!(v.inst, Instruction::Drop { .. })).count();
        assert!(drop_inst_count >= 2, "Expected at least child_future drop and local resource drop, got {}", drop_inst_count);

        // Verify box free exists in drop_fn
        let box_free_count = drop_fn.values.iter().filter(|v| matches!(v.inst, Instruction::BoxFree { .. })).count();
        assert_eq!(box_free_count, 1);

        assert!(mellis_optimizer::verify_module(&module).is_ok());
    }
}


