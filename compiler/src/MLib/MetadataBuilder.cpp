#include "mellis/MLib/MetadataBuilder.h"
#include <algorithm>

#include <unordered_set>
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/ProgramNode.h"

namespace fl {
namespace mlib {

MetadataBuilder::MetadataBuilder(StringTableBuilder& stringTable)
    : stringTable(stringTable) {}

void MetadataBuilder::buildFromSnapshot(const SemanticSnapshot& snapshot) {
    snapshot_ = &snapshot;
    const auto& symTab = snapshot.getSymbolTable();
    std::vector<const Symbol*> exports;
    for (uint32_t i = 0; i < symTab.symbolCount(); ++i) {
        const auto& sym = symTab.getSymbol(i);
        // [DEBUG]
        std::cout << "[MLibGen Debug] sym=" << sym.name.view() << " vis=" << (int)sym.visibility << " ext=" << sym.isExternal << " kind=" << (int)sym.kind << "\n";
        
        if (sym.visibility == Visibility::Public) {
            bool isChildOfImplOrTrait = false;
            if (sym.declaredInScope != kInvalidScopeID) {
                auto kindValue = static_cast<uint8_t>(symTab.getScope(sym.declaredInScope).kind);
                if (kindValue == static_cast<uint8_t>(fl::ScopeKind::Trait) || 
                    kindValue == static_cast<uint8_t>(fl::ScopeKind::Impl) || 
                    kindValue == static_cast<uint8_t>(fl::ScopeKind::Enum)) {
                    isChildOfImplOrTrait = true;
                }
            }
            if (!isChildOfImplOrTrait) {
                exports.push_back(&sym);
            }
        }
    }
    std::sort(exports.begin(), exports.end(), [](const Symbol* a, const Symbol* b) {
        return a->name.view() < b->name.view();
    });

    std::unordered_set<const Symbol*> visited;
    std::unordered_map<const Symbol*, uint32_t> symbolToFunctionId;
    
    auto visit = [&](auto& self, const Symbol* sym) -> void {
        if (!sym || visited.count(sym)) return;
        visited.insert(sym);

        if (sym->kind == SymbolKind::Struct) {
            addType(std::string(sym->name.view()), 0, 0, 0); // We will refine this
        } else if (sym->kind == SymbolKind::Function) {
            uint32_t sigTypeId = const_cast<MetadataBuilder*>(this)->addTypeRef(snapshot.typeOf(sym->id));
            std::cout << "[MLibGen Debug] addFunction called for " << sym->name.view() << " sigTypeId=" << sigTypeId << "\n";
            uint32_t fid = addFunction(std::string(sym->name.view()), 0, sigTypeId);
            symbolToFunctionId[sym] = fid;
        } else if (sym->kind == SymbolKind::Trait) {
            addTrait(std::string(sym->name.view()), 0);
        } else if (sym->kind == SymbolKind::Enum) {
            addType(std::string(sym->name.view()), 0, 0, 0);
        } else if (sym->kind == SymbolKind::TypeAlias) {
            addType(std::string(sym->name.view()), 0, 0, 0); // Export TypeAlias as type
        } else if (sym->kind == SymbolKind::Module) {
            addNamespace(std::string(sym->name.view()), 0);
        }
    };

    for (const Symbol* sym : exports) {
        visit(visit, sym);
    }
    
    if (snapshot.getProgram()) {
        auto collectImpls = [&](auto& self, const DeclNode* decl) -> void {
            if (!decl) return;
            if (auto* mod = dynamic_cast<const ModDeclNode*>(decl)) {
                for (const auto& item : mod->decls) {
                    self(self, item.get());
                }
            } else if (auto* impl = dynamic_cast<const ImplDeclNode*>(decl)) {
                // Determine visibility by checking if the target type is public.
                // For now, we export all impls if they are for public types, or we just export all impls and let consumer figure it out.
                // In a real module system, impls are exported if their target type or trait is public.
                ImplEntry entry;
                entry.selfTypeRefID = const_cast<MetadataBuilder*>(this)->addTypeRef(snapshot.typeOf(impl->bodyScopeId)); // Need semantic type of the impl body!
                entry.traitRefID = 0xFFFFFFFF; // TODO: Trait Ref
                entry.genericParamCount = static_cast<uint16_t>(impl->genericParams.size());
                entry.methodCount = 0; // TODO
                entry.associatedTypeCount = 0; // TODO
                entry.boundCount = 0; // TODO
                entry.payloadSize = 0; // TODO
                const_cast<MetadataBuilder*>(this)->addImpl(entry);
            }
        };
        for (const auto& item : snapshot.getProgram()->items) {
            collectImpls(collectImpls, dynamic_cast<const DeclNode*>(item.get()));
        }
    }
}

uint32_t MetadataBuilder::addNamespace(const std::string& name, uint32_t parentNamespaceID) {
    uint32_t id = static_cast<uint32_t>(namespaces.size());
    InternalNamespace in;
    in.name = name;
    in.entry.parentNamespaceID = parentNamespaceID;
    namespaces.push_back(in);
    stringTable.addString(name);
    return id;
}

uint32_t MetadataBuilder::addType(const std::string& name, uint32_t namespaceID, uint64_t size, uint64_t alignment) {
    uint32_t id = static_cast<uint32_t>(types.size());
    InternalType in;
    in.name = name;
    in.entry.namespaceID = namespaceID;
    in.entry.size = size;
    in.entry.alignment = alignment;
    in.entry.visibility = static_cast<uint8_t>(Visibility::Public);
    in.entry.moduleID = 0;
    types.push_back(in);
    stringTable.addString(name);
    return id;
}

uint32_t MetadataBuilder::addTrait(const std::string& name, uint32_t namespaceID) {
    uint32_t id = static_cast<uint32_t>(traits.size());
    InternalTrait in;
    in.name = name;
    in.entry.namespaceID = namespaceID;
    in.entry.visibility = static_cast<uint8_t>(Visibility::Public);
    in.entry.moduleID = 0;
    traits.push_back(in);
    stringTable.addString(name);
    return id;
}

uint32_t MetadataBuilder::addFunction(const std::string& name, uint32_t namespaceID, uint32_t signatureTypeID) {
    uint32_t id = static_cast<uint32_t>(functions.size());
    InternalFunction in;
    in.name = name;
    in.entry.namespaceID = namespaceID;
    in.entry.signatureTypeID = signatureTypeID;
    in.entry.isVariadic = 0;
    in.entry.paramCount = 0;
    in.entry.flags = 0;
    in.entry.visibility = static_cast<uint8_t>(Visibility::Public);
    in.entry.moduleID = 0;
    functions.push_back(in);
    stringTable.addString(name);
    return id;
}

uint32_t MetadataBuilder::addImpl(const ImplEntry& entry) {
    uint32_t id = static_cast<uint32_t>(impls.size());
    impls.push_back(entry);
    return id;
}

void MetadataBuilder::serializeMetadata(BinaryWriter& writer) const {
    writer.writeU32(static_cast<uint32_t>(namespaces.size()));
    for (const auto& in : namespaces) {
        NamespaceEntry e = in.entry;
        e.nameStringID = stringTable.getStringOffset(in.name);
        writer.writeStruct(e);
    }

    writer.writeU32(static_cast<uint32_t>(types.size()));
    for (const auto& in : types) {
        TypeEntry e = in.entry;
        e.nameStringID = stringTable.getStringOffset(in.name);
        writer.writeStruct(e);
    }

    writer.writeU32(static_cast<uint32_t>(traits.size()));
    for (const auto& in : traits) {
        TraitEntry e = in.entry;
        e.nameStringID = stringTable.getStringOffset(in.name);
        writer.writeStruct(e);
    }

    std::cout << "[MLibGen Debug] Writing " << functions.size() << " functions\n";
    writer.writeU32(static_cast<uint32_t>(functions.size()));
    for (const auto& in : functions) {
        FunctionEntry e = in.entry;
        e.nameStringID = stringTable.getStringOffset(in.name);
        writer.writeStruct(e);
    }
}

void MetadataBuilder::serializeImpls(BinaryWriter& writer) const {
    writer.writeU32(static_cast<uint32_t>(impls.size()));
    for (const auto& entry : impls) writer.writeStruct(entry);
}

void MetadataBuilder::serializeTypeRefs(BinaryWriter& writer) const {
    std::cout << "[MLibGen Debug] Writing " << typeRefs.size() << " typeRefs:\n";
    for (size_t i = 0; i < typeRefs.size(); ++i) {
        std::cout << "  [" << i << "] Kind=" << (int)typeRefs[i].record.kind 
                  << " psize=" << typeRefs[i].record.payloadSize 
                  << " actual_psize=" << typeRefs[i].payload.size() * 4
                  << " words=[";
        for (auto w : typeRefs[i].payload) {
            std::cout << w << ", ";
        }
        std::cout << "]\n";
    }

    writer.writeU32(static_cast<uint32_t>(typeRefs.size()));
    for (const auto& ir : typeRefs) {
        writer.writeStruct(ir.record);
        for (uint32_t val : ir.payload) {
            writer.writeU32(val);
        }
    }
}


uint32_t MetadataBuilder::addTypeRef(const fl::Type* type) {
    if (!type) return 0xFFFFFFFF;
    
    uint32_t id = static_cast<uint32_t>(typeRefs.size());
    typeRefs.push_back(InternalTypeRef{}); // Reserve slot

    InternalTypeRef ir;
    ir.record.flags = 0;
    ir.record.payloadSize = 0;

    switch (type->getKind()) {
        case TypeKind::Primitive: {
            ir.record.kind = TypeRefKind::Primitive;
            ir.payload.push_back(static_cast<uint32_t>(static_cast<const PrimitiveType*>(type)->builtinKind));
            break;
        }
        case TypeKind::Struct: {
            auto* st = static_cast<const StructType*>(type);
            ir.record.kind = TypeRefKind::Named;
            std::string name;
            if (snapshot_) {
                const auto& sym = snapshot_->getSymbolTable().getSymbol(st->structSymbolId);
                name = std::string(sym.name.view());
            }
            stringTable.addString(name);
            ir.payload.push_back(stringTable.getStringOffset(name));
            ir.payload.push_back(static_cast<uint32_t>(st->genericArgs.size()));
            for (auto* arg : st->genericArgs) {
                ir.payload.push_back(addTypeRef(arg));
            }
            break;
        }
        case TypeKind::Enum: {
            auto* et = static_cast<const EnumType*>(type);
            ir.record.kind = TypeRefKind::Named;
            std::string name;
            if (snapshot_) {
                const auto& sym = snapshot_->getSymbolTable().getSymbol(et->enumSymbolId);
                name = std::string(sym.name.view());
            }
            stringTable.addString(name);
            ir.payload.push_back(stringTable.getStringOffset(name));
            ir.payload.push_back(static_cast<uint32_t>(et->genericArgs.size()));
            for (auto* arg : et->genericArgs) {
                ir.payload.push_back(addTypeRef(arg));
            }
            break;
        }
        case TypeKind::Pointer: {
            auto* pt = static_cast<const PointerType*>(type);
            ir.record.kind = TypeRefKind::Pointer;
            if (pt->isMutable) ir.record.flags |= 1;
            ir.payload.push_back(addTypeRef(pt->pointee));
            break;
        }
        case TypeKind::Reference: {
            auto* rt = static_cast<const ReferenceType*>(type);
            ir.record.kind = TypeRefKind::Reference;
            if (rt->isMutable) ir.record.flags |= 1;
            ir.payload.push_back(addTypeRef(rt->pointee));
            break;
        }
        case TypeKind::GenericParam: {
            auto* gt = static_cast<const GenericParamType*>(type);
            ir.record.kind = TypeRefKind::GenericParam;
            ir.payload.push_back(gt->paramId);
            stringTable.addString(std::string(gt->name));
            ir.payload.push_back(stringTable.getStringOffset(std::string(gt->name)));
            break;
        }
        case TypeKind::Trait: {
            auto* tt = static_cast<const TraitType*>(type);
            ir.record.kind = TypeRefKind::Named;
            std::string name;
            if (snapshot_) {
                const auto& sym = snapshot_->getSymbolTable().getSymbol(tt->traitId);
                name = std::string(sym.name.view());
            }
            stringTable.addString(name);
            ir.payload.push_back(stringTable.getStringOffset(name));
            ir.payload.push_back(0);
            break;
        }
        case TypeKind::Array: {
            auto* at = static_cast<const ArrayType*>(type);
            ir.record.kind = TypeRefKind::Array;
            ir.payload.push_back(addTypeRef(at->elementType));
            ir.payload.push_back(static_cast<uint32_t>(at->length));
            break;
        }
        case TypeKind::Slice: {
            auto* st = static_cast<const SliceType*>(type);
            ir.record.kind = TypeRefKind::Slice;
            ir.payload.push_back(addTypeRef(st->elementType));
            break;
        }
        case TypeKind::Tuple: {
            auto* tt = static_cast<const TupleType*>(type);
            ir.record.kind = TypeRefKind::Tuple;
            ir.payload.push_back(static_cast<uint32_t>(tt->elements.size()));
            for (auto* el : tt->elements) {
                ir.payload.push_back(addTypeRef(el));
            }
            break;
        }
        case TypeKind::Function: {
            auto* ft = static_cast<const FunctionType*>(type);
            ir.record.kind = TypeRefKind::Function;
            ir.payload.push_back(addTypeRef(ft->returnType));
            ir.payload.push_back(ft->isVariadic ? 1 : 0);
            ir.payload.push_back(static_cast<uint32_t>(ft->paramTypes.size()));
            for (auto* pt : ft->paramTypes) {
                ir.payload.push_back(addTypeRef(pt));
            }
            break;
        }
        default:
            ir.record.kind = TypeRefKind::Primitive;
            ir.payload.push_back(0); // placeholder for unhandled
            break;
    }
    
    ir.record.payloadSize = static_cast<uint16_t>(ir.payload.size() * sizeof(uint32_t));
    typeRefs[id] = ir;
    return id;
}

} // namespace mlib
} // namespace fl

