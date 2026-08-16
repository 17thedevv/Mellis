#include "mellis/IR/IRVerifier.h"
#include <unordered_set>
#include <unordered_map>
#include "mellis/MiddleEnd/SemanticSnapshot.h"

namespace fl {

IRVerificationResult IRVerifier::verify(const mvir::Module& module) {
    auto res = verifySemanticClosure(module);
    if (!res.ok) return res;

    for (const auto& func : module.functions) {
        if (!func) continue;
        auto funcRes = verifyFunction(*func);
        if (!funcRes.ok) return funcRes;
    }
    return {true, ""};
}

IRVerificationResult IRVerifier::verifyFunction(const mvir::Function& function) {
    if (function.blocks.empty()) {
        return {true, ""}; // External or empty functions are ok
    }

    auto res = verifyEntryBlock(function);
    if (!res.ok) return res;

    res = verifyTerminators(function);
    if (!res.ok) return res;

    res = verifyReachability(function);
    if (!res.ok) return res;

    res = verifyAllocaPlacement(function);
    if (!res.ok) return res;

    res = verifyDominance(function);
    if (!res.ok) return res;

    res = verifyBranchTargets(function);
    if (!res.ok) return res;

    res = verifyTypeConsistency(function);
    if (!res.ok) return res;

    res = verifyBorrowTargets(function);
    if (!res.ok) return res;

    res = verifyDropTargets(function);
    if (!res.ok) return res;

    return {true, ""};
}

IRVerificationResult IRVerifier::verifyEntryBlock(const mvir::Function& function) {
    // Entry block is the first block, it should not have predecessors.
    if (function.blocks.empty()) return {true, ""};
    
    std::string entryLabel = function.blocks[0]->label.name;
    
    for (const auto& bb : function.blocks) {
        if (!bb->terminator) continue;
        
        auto opcode = bb->terminator->getOpcode();
        if (opcode == mvir::Opcode::Jump) {
            auto* jmp = static_cast<const mvir::JumpTerm*>(bb->terminator.get());
            if (jmp->target.name == entryLabel) {
                return {false, "Entry block has predecessor from block " + bb->label.name};
            }
        } else if (opcode == mvir::Opcode::Branch) {
            auto* br = static_cast<const mvir::BranchTerm*>(bb->terminator.get());
            if (br->trueTarget.name == entryLabel || br->falseTarget.name == entryLabel) {
                return {false, "Entry block has predecessor from block " + bb->label.name};
            }
        } else if (opcode == mvir::Opcode::Switch) {
            auto* sw = static_cast<const mvir::SwitchTerm*>(bb->terminator.get());
            if (sw->defaultTarget.name == entryLabel) {
                return {false, "Entry block has predecessor from block " + bb->label.name};
            }
            for (const auto& c : sw->cases) {
                if (c.second.name == entryLabel) {
                    return {false, "Entry block has predecessor from block " + bb->label.name};
                }
            }
        }
    }
    
    return {true, ""};
}

IRVerificationResult IRVerifier::verifyTerminators(const mvir::Function& function) {
    for (const auto& bb : function.blocks) {
        if (!bb->terminator) {
            return {false, "Basic block " + bb->label.name + " is missing a terminator"};
        }
    }
    return {true, ""};
}

IRVerificationResult IRVerifier::verifyReachability(const mvir::Function& function) {
    if (function.blocks.empty()) return {true, ""};

    std::unordered_set<std::string> visited;
    std::vector<std::string> stack;

    auto addTarget = [&](const mvir::LabelId& target) {
        if (visited.find(target.name) == visited.end()) {
            visited.insert(target.name);
            stack.push_back(target.name);
        }
    };

    // Entry block is always reachable
    addTarget(function.blocks[0]->label);

    std::unordered_map<std::string, const mvir::BasicBlock*> blockMap;
    for (const auto& bb : function.blocks) {
        blockMap[bb->label.name] = bb.get();
    }

    while (!stack.empty()) {
        std::string curr = stack.back();
        stack.pop_back();

        auto it = blockMap.find(curr);
        if (it == blockMap.end()) continue;

        const auto* bb = it->second;
        if (!bb->terminator) continue;

        auto opcode = bb->terminator->getOpcode();
        if (opcode == mvir::Opcode::Jump) {
            auto* jmp = static_cast<const mvir::JumpTerm*>(bb->terminator.get());
            addTarget(jmp->target);
        } else if (opcode == mvir::Opcode::Branch) {
            auto* br = static_cast<const mvir::BranchTerm*>(bb->terminator.get());
            addTarget(br->trueTarget);
            addTarget(br->falseTarget);
        } else if (opcode == mvir::Opcode::Switch) {
            auto* sw = static_cast<const mvir::SwitchTerm*>(bb->terminator.get());
            addTarget(sw->defaultTarget);
            for (const auto& c : sw->cases) {
                addTarget(c.second);
            }
        }
    }

    for (const auto& bb : function.blocks) {
        if (visited.find(bb->label.name) == visited.end()) {
            return {false, "Basic block " + bb->label.name + " is unreachable"};
        }
    }

    return {true, ""};
}

IRVerificationResult IRVerifier::verifyAllocaPlacement(const mvir::Function& function) {
    if (function.blocks.empty()) return {true, ""};
    
    // Check blocks other than the entry block for LocalInst
    for (size_t i = 1; i < function.blocks.size(); ++i) {
        const auto& bb = function.blocks[i];
        for (const auto& inst : bb->instructions) {
            if (inst->getOpcode() == mvir::Opcode::Local) {
                return {false, "LocalInst found outside of entry block in " + bb->label.name};
            }
        }
    }
    return {true, ""};
}

IRVerificationResult IRVerifier::verifyDominance(const mvir::Function& function) {
    // A simplified dominance check: temporaries must be defined before use in the same block,
    // or defined in a block that strictly dominates the current block.
    // For MVIR, since we generate SSA-like temps without Phi nodes (by using locals for mutable vars),
    // every temporary must be defined globally before use.
    // Since we verified no cycles above (if we did) and reachability, a simple global definition check
    // might suffice for MVIR temporaries.
    // Let's implement a global def-use check.
    
    std::unordered_set<std::string> definedLocals;
    for (const auto& param : function.params) {
        definedLocals.insert(param.id.toString());
    }

    // First pass: collect all definitions. (Strict dominance requires CFG analysis, 
    // but for now we just verify no use before def in a linear scan if CFG is a DAG, 
    // or just that they are defined somewhere).
    for (const auto& bb : function.blocks) {
        for (const auto& inst : bb->instructions) {
            auto opcode = inst->getOpcode();
            // Collect defined IDs (dest)
            if (opcode == mvir::Opcode::Local) {
                definedLocals.insert(static_cast<const mvir::LocalInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::Alu) {
                definedLocals.insert(static_cast<const mvir::AluInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::Load) {
                definedLocals.insert(static_cast<const mvir::LoadInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::Borrow) {
                definedLocals.insert(static_cast<const mvir::BorrowInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::Cast) {
                definedLocals.insert(static_cast<const mvir::CastInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::Sizeof) {
                definedLocals.insert(static_cast<const mvir::SizeofInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::Alignof) {
                definedLocals.insert(static_cast<const mvir::AlignofInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::Unary) {
                definedLocals.insert(static_cast<const mvir::UnaryInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::Extract) {
                definedLocals.insert(static_cast<const mvir::ExtractInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::Tag) {
                definedLocals.insert(static_cast<const mvir::TagInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::Variant) {
                definedLocals.insert(static_cast<const mvir::VariantInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::MakeTraitObject) {
                definedLocals.insert(static_cast<const mvir::MakeTraitObjectInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::Call) {
                if (static_cast<const mvir::CallInst*>(inst.get())->dest) {
                    definedLocals.insert(static_cast<const mvir::CallInst*>(inst.get())->dest->toString());
                }
            } else if (opcode == mvir::Opcode::VirtualCall) {
                if (static_cast<const mvir::VirtualCallInst*>(inst.get())->dest) {
                    definedLocals.insert(static_cast<const mvir::VirtualCallInst*>(inst.get())->dest->toString());
                }
            } else if (opcode == mvir::Opcode::HeapAlloc) {
                definedLocals.insert(static_cast<const mvir::HeapAllocInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::Await) {
                definedLocals.insert(static_cast<const mvir::AwaitInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::PtrOffset) {
                definedLocals.insert(static_cast<const mvir::PtrOffsetInst*>(inst.get())->dest.toString());
            } else if (opcode == mvir::Opcode::PtrDiff) {
                definedLocals.insert(static_cast<const mvir::PtrDiffInst*>(inst.get())->dest.toString());
            }
        }
    }

    auto checkOperand = [&](const mvir::Operand& op, const std::string& instName) -> IRVerificationResult {
        if (auto* loc = mvir::getLocalIf(op)) {
            if (definedLocals.find(loc->toString()) == definedLocals.end()) {
                return {false, "Use of undefined temporary/local " + loc->toString() + " in " + instName};
            }
        }
        return {true, ""};
    };

    for (const auto& bb : function.blocks) {
        for (const auto& inst : bb->instructions) {
            auto opcode = inst->getOpcode();
            IRVerificationResult res = {true, ""};
            
            if (opcode == mvir::Opcode::Store) {
                auto* store = static_cast<const mvir::StoreInst*>(inst.get());
                res = checkOperand(store->value, "StoreInst");
                if (res.ok) res = checkOperand(store->ptr, "StoreInst");
            } else if (opcode == mvir::Opcode::Load) {
                res = checkOperand(static_cast<const mvir::LoadInst*>(inst.get())->ptr, "LoadInst");
            } else if (opcode == mvir::Opcode::Alu) {
                auto* alu = static_cast<const mvir::AluInst*>(inst.get());
                res = checkOperand(alu->left, "AluInst");
                if (res.ok) res = checkOperand(alu->right, "AluInst");
            } else if (opcode == mvir::Opcode::Borrow) {
                res = checkOperand(static_cast<const mvir::BorrowInst*>(inst.get())->base, "BorrowInst");
            } else if (opcode == mvir::Opcode::Cast) {
                res = checkOperand(static_cast<const mvir::CastInst*>(inst.get())->value, "CastInst");
            } else if (opcode == mvir::Opcode::Drop) {
                res = checkOperand(static_cast<const mvir::DropInst*>(inst.get())->value, "DropInst");
            } else if (opcode == mvir::Opcode::Unary) {
                res = checkOperand(static_cast<const mvir::UnaryInst*>(inst.get())->operand, "UnaryInst");
            } else if (opcode == mvir::Opcode::Extract) {
                res = checkOperand(static_cast<const mvir::ExtractInst*>(inst.get())->base, "ExtractInst");
            } else if (opcode == mvir::Opcode::Tag) {
                res = checkOperand(static_cast<const mvir::TagInst*>(inst.get())->base, "TagInst");
            } else if (opcode == mvir::Opcode::Call) {
                auto* call = static_cast<const mvir::CallInst*>(inst.get());
                for (const auto& arg : call->args) {
                    if (!res.ok) break;
                    res = checkOperand(arg, "CallInst");
                }
            } else if (opcode == mvir::Opcode::VirtualCall) {
                auto* vcall = static_cast<const mvir::VirtualCallInst*>(inst.get());
                res = checkOperand(vcall->receiver, "VirtualCallInst");
                for (const auto& arg : vcall->args) {
                    if (!res.ok) break;
                    res = checkOperand(arg, "VirtualCallInst");
                }
            } else if (opcode == mvir::Opcode::PtrOffset) {
                auto* poff = static_cast<const mvir::PtrOffsetInst*>(inst.get());
                res = checkOperand(poff->ptr, "PtrOffsetInst");
                if (res.ok) res = checkOperand(poff->offset, "PtrOffsetInst");
            } else if (opcode == mvir::Opcode::PtrDiff) {
                auto* pdiff = static_cast<const mvir::PtrDiffInst*>(inst.get());
                res = checkOperand(pdiff->left, "PtrDiffInst");
                if (res.ok) res = checkOperand(pdiff->right, "PtrDiffInst");
            }
            if (!res.ok) return res;
        }
        
        if (bb->terminator) {
            auto opcode = bb->terminator->getOpcode();
            if (opcode == mvir::Opcode::Branch) {
                auto* br = static_cast<const mvir::BranchTerm*>(bb->terminator.get());
                auto res = checkOperand(br->condition, "BranchTerm");
                if (!res.ok) return res;
            } else if (opcode == mvir::Opcode::Ret) {
                auto* ret = static_cast<const mvir::RetTerm*>(bb->terminator.get());
                if (ret->value.has_value()) {
                    auto res = checkOperand(ret->value.value(), "RetTerm");
                    if (!res.ok) return res;
                }
            } else if (opcode == mvir::Opcode::Switch) {
                auto* sw = static_cast<const mvir::SwitchTerm*>(bb->terminator.get());
                auto res = checkOperand(sw->condition, "SwitchTerm");
                if (!res.ok) return res;
            }
        }
    }

    return {true, ""};
}

IRVerificationResult IRVerifier::verifyBranchTargets(const mvir::Function& function) {
    std::unordered_set<std::string> validLabels;
    for (const auto& bb : function.blocks) {
        validLabels.insert(bb->label.name);
    }

    for (const auto& bb : function.blocks) {
        if (!bb->terminator) continue;
        auto opcode = bb->terminator->getOpcode();
        if (opcode == mvir::Opcode::Jump) {
            auto* jmp = static_cast<const mvir::JumpTerm*>(bb->terminator.get());
            if (validLabels.find(jmp->target.name) == validLabels.end()) {
                return {false, "Jump to invalid block " + jmp->target.name};
            }
        } else if (opcode == mvir::Opcode::Branch) {
            auto* br = static_cast<const mvir::BranchTerm*>(bb->terminator.get());
            if (validLabels.find(br->trueTarget.name) == validLabels.end()) {
                return {false, "Branch trueTarget to invalid block " + br->trueTarget.name};
            }
            if (validLabels.find(br->falseTarget.name) == validLabels.end()) {
                return {false, "Branch falseTarget to invalid block " + br->falseTarget.name};
            }
        } else if (opcode == mvir::Opcode::Switch) {
            auto* sw = static_cast<const mvir::SwitchTerm*>(bb->terminator.get());
            if (validLabels.find(sw->defaultTarget.name) == validLabels.end()) {
                return {false, "Switch defaultTarget to invalid block " + sw->defaultTarget.name};
            }
            for (const auto& c : sw->cases) {
                if (validLabels.find(c.second.name) == validLabels.end()) {
                    return {false, "Switch case target to invalid block " + c.second.name};
                }
            }
        }
    }

    return {true, ""};
}

IRVerificationResult IRVerifier::verifyTypeConsistency(const mvir::Function& function) {
    // Not implemented fully yet. We just return true for now.
    // In a complete verifier, we'd check if `AluInst::left->type == AluInst::right->type` etc.
    return {true, ""};
}

IRVerificationResult IRVerifier::verifyBorrowTargets(const mvir::Function& function) {
    // Ensure BorrowInst targets existing locals.
    return {true, ""};
}

IRVerificationResult IRVerifier::verifyDropTargets(const mvir::Function& function) {
    // Drop targets must be defined. Handled by dominance basically.
    return {true, ""};
}

IRVerificationResult IRVerifier::verifyCallSignatures(const mvir::Module& module, const mvir::Function& function) {
    for (const auto& bb : function.blocks) {
        for (const auto& inst : bb->instructions) {
            if (inst->getOpcode() == mvir::Opcode::Call) {
                auto* call = static_cast<const mvir::CallInst*>(inst.get());
                for (const auto& f : module.functions) {
                    if (auto* glob = mvir::getGlobalIf(call->func)) {
                        if (f->name == *glob) {
                            break;
                        }
                    }
                }
            }
        }
    }
    return {true, ""};
}

IRVerificationResult IRVerifier::verifyTypeSemanticClosure(const Type* t, const std::string& context) {
    if (!t) return {true, ""};
    if (t->getKind() == TypeKind::InferenceVar) return {false, context + " contains InferenceVar"};
    if (t->getKind() == TypeKind::Unknown) return {false, context + " contains Unknown type"};
    if (t->getKind() == TypeKind::Error) return {false, context + " contains Error type"};
    if (t->getKind() == TypeKind::AssociatedProjection) return {false, context + " contains AssociatedProjection"};
    
    switch (t->getKind()) {
        case TypeKind::Struct: {
            SymbolID id = static_cast<const StructType*>(t)->structSymbolId;
            if (!snapshot_->typeOf(id)) return {false, context + " structSymbolId " + std::to_string(id) + " unresolved in SemanticSnapshot"};
            break;
        }
        case TypeKind::Enum: {
            SymbolID id = static_cast<const EnumType*>(t)->enumSymbolId;
            if (!snapshot_->typeOf(id)) return {false, context + " enumSymbolId " + std::to_string(id) + " unresolved in SemanticSnapshot"};
            break;
        }
        case TypeKind::Trait: {
            SymbolID id = static_cast<const TraitType*>(t)->traitId;
            if (!snapshot_->typeOf(id)) return {false, context + " traitId " + std::to_string(id) + " unresolved in SemanticSnapshot"};
            break;
        }
        case TypeKind::Function: {
            const auto* fnType = static_cast<const FunctionType*>(t);
            for (const auto* p : fnType->paramTypes) {
                auto res = verifyTypeSemanticClosure(p, context + " (param)");
                if (!res.ok) return res;
            }
            return verifyTypeSemanticClosure(fnType->returnType, context + " (return)");
        }
        case TypeKind::Pointer: return verifyTypeSemanticClosure(static_cast<const PointerType*>(t)->pointee, context + " (pointer)");
        case TypeKind::Reference: {
            const auto* ref = static_cast<const ReferenceType*>(t);
            if (ref->lifetime.kind == LifetimeKind::Inference) {
                return {false, context + " contains InferenceLifetime"};
            }
            return verifyTypeSemanticClosure(ref->pointee, context + " (reference)");
        }
        case TypeKind::Array: return verifyTypeSemanticClosure(static_cast<const ArrayType*>(t)->elementType, context + " (array)");
        case TypeKind::Slice: return verifyTypeSemanticClosure(static_cast<const SliceType*>(t)->elementType, context + " (slice)");
        case TypeKind::Tuple: {
            for (const auto* e : static_cast<const TupleType*>(t)->elements) {
                auto res = verifyTypeSemanticClosure(e, context + " (tuple)");
                if (!res.ok) return res;
            }
            break;
        }
        default: break;
    }
    return {true, ""};
}

IRVerificationResult IRVerifier::verifySemanticClosure(const mvir::Module& module) {
    if (!snapshot_) return {true, ""}; // Skip if no snapshot available
    
    for (const auto& func : module.functions) {
        if (!func) continue;
        
        std::string ctx = "Function " + func->name.name;
        
        if (func->name.symbolId == 0) return {false, ctx + " missing symbolId"};
        
        if (func->name.symbolId >= snapshot_->getSymbolTable().symbolCount()) {
            return {false, ctx + " symbolId " + std::to_string(func->name.symbolId) + " unresolved in SemanticSnapshot"};
        }
        
        auto res = verifyTypeSemanticClosure(func->returnType, ctx + " return type");
        if (!res.ok) return res;

        for (const auto& p : func->params) {
            res = verifyTypeSemanticClosure(p.type, ctx + " param " + p.id.name);
            if (!res.ok) return res;
            if (p.id.symbolId != 0 && p.id.symbolId >= snapshot_->getSymbolTable().symbolCount()) {
                return {false, ctx + " param " + p.id.name + " symbolId " + std::to_string(p.id.symbolId) + " unresolved"};
            }
        }
        
        for (const auto& bb : func->blocks) {
            for (const auto& inst : bb->instructions) {
                std::string instCtx = ctx + " bb " + bb->label.name + " instruction " + std::to_string(static_cast<int>(inst->getOpcode()));
                
                if (auto* loc = dynamic_cast<const mvir::LocalInst*>(inst.get())) {
                    auto r = verifyTypeSemanticClosure(loc->type, instCtx + " LocalInst");
                    if (!r.ok) return r;
                    if (loc->dest.symbolId != fl::kInvalidSymbolID && loc->dest.symbolId >= snapshot_->getSymbolTable().symbolCount()) {
                        return {false, instCtx + " dest symbolId " + std::to_string(loc->dest.symbolId) + " unresolved"};
                    }
                } else if (auto* ld = dynamic_cast<const mvir::LoadInst*>(inst.get())) {
                    auto r = verifyTypeSemanticClosure(ld->type, instCtx + " LoadInst");
                    if (!r.ok) return r;
                } else if (auto* st = dynamic_cast<const mvir::StoreInst*>(inst.get())) {
                    auto r = verifyTypeSemanticClosure(st->type, instCtx + " StoreInst");
                    if (!r.ok) return r;
                } else if (auto* cast = dynamic_cast<const mvir::CastInst*>(inst.get())) {
                    auto r = verifyTypeSemanticClosure(cast->targetType, instCtx + " CastInst targetType");
                    if (!r.ok) return r;
                } else if (auto* mk = dynamic_cast<const mvir::MakeTraitObjectInst*>(inst.get())) {
                    auto r = verifyTypeSemanticClosure(mk->targetType, instCtx + " MakeTraitObjectInst targetType");
                    if (!r.ok) return r;
                    auto cr = verifyTypeSemanticClosure(mk->concreteType, instCtx + " MakeTraitObjectInst concreteType");
                    if (!cr.ok) return cr;
                } else if (auto* call = dynamic_cast<const mvir::CallInst*>(inst.get())) {
                    auto r = verifyTypeSemanticClosure(call->funcType, instCtx + " CallInst funcType");
                    if (!r.ok) return r;
                    if (auto* glob = mvir::getGlobalIf(call->func)) {
                        if (glob->symbolId != 0 && glob->symbolId >= snapshot_->getSymbolTable().symbolCount()) {
                             return {false, instCtx + " call target symbolId " + std::to_string(glob->symbolId) + " unresolved"};
                        }
                    }
                } else if (auto* vcall = dynamic_cast<const mvir::VirtualCallInst*>(inst.get())) {
                    auto r = verifyTypeSemanticClosure(vcall->traitType, instCtx + " VirtualCallInst traitType");
                    if (!r.ok) return r;
                    r = verifyTypeSemanticClosure(vcall->methodType, instCtx + " VirtualCallInst methodType");
                    if (!r.ok) return r;
                } else if (auto* drop = dynamic_cast<const mvir::DropInst*>(inst.get())) {
                    auto r = verifyTypeSemanticClosure(drop->type, instCtx + " DropInst type");
                    if (!r.ok) return r;
                } else if (auto* sz = dynamic_cast<const mvir::SizeofInst*>(inst.get())) {
                    auto r = verifyTypeSemanticClosure(sz->targetType, instCtx + " SizeofInst targetType");
                    if (!r.ok) return r;
                } else if (auto* al = dynamic_cast<const mvir::AlignofInst*>(inst.get())) {
                    auto r = verifyTypeSemanticClosure(al->targetType, instCtx + " AlignofInst targetType");
                    if (!r.ok) return r;
                } else if (auto* ext = dynamic_cast<const mvir::ExtractInst*>(inst.get())) {
                    for (const auto* t : ext->payloadTypes) {
                        auto r = verifyTypeSemanticClosure(t, instCtx + " ExtractInst payloadType");
                        if (!r.ok) return r;
                    }
                } else if (auto* var = dynamic_cast<const mvir::VariantInst*>(inst.get())) {
                    auto r = verifyTypeSemanticClosure(var->enumType, instCtx + " VariantInst enumType");
                    if (!r.ok) return r;
                }
            }
        }
    }
    return {true, ""};
}

} // namespace fl
