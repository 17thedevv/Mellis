#include "mellis/FrontEnd/LifetimeElider.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/TypeNode.h"
#include "mellis/AST/StmtNode.h"

namespace fl {

LifetimeElider::LifetimeElider(DiagnosticEngine& diag) : diag(diag) {}

std::string LifetimeElider::generateAnonymousLifetime() {
    return "'_" + std::to_string(lifetimeCounter++);
}

void LifetimeElider::elide(ProgramNode& node) {
    node.accept(*this);
}

void LifetimeElider::visit(ProgramNode& node) {
    for (auto& item : node.items) {
        item->accept(*this);
    }
}

void LifetimeElider::visit(FunctionDeclNode& node) {
    // Reset state for this function
    currentInputLifetimes.clear();
    lifetimeCounter = 1;
    
    // First, collect explicitly declared lifetimes in this function
    std::vector<std::string> declaredLifetimes;
    for (const auto& gp : node.genericParams) {
        if (gp.kind == GenericParamKind::Lifetime) {
            declaredLifetimes.push_back(std::string(gp.name));
        }
    }
    
    // Process parameters
    inInputParameters = true;
    for (auto& param : node.params) {
        param->accept(*this);
    }
    inInputParameters = false;
    
    // Determine the elided output lifetime
    std::string elidedOutputLifetime = "";
    if (currentInputLifetimes.size() == 1) {
        elidedOutputLifetime = std::string(currentInputLifetimes[0]->name);
    } else if (currentInputLifetimes.size() > 1) {
        // Look for 'self' parameter.
        // Mellis currently doesn't have a rigid 'self' parameter name internally enforced,
        // but typically the first parameter in an impl method is 'self'.
        bool foundSelf = false;
        if (!node.params.empty()) {
            if (node.params[0]->name == "self") {
                // Determine if it has a reference
                auto* refType = dynamic_cast<ReferenceTypeNode*>(node.params[0]->type.get());
                if (refType && refType->lifetime) {
                    elidedOutputLifetime = std::string(refType->lifetime->name);
                    foundSelf = true;
                }
            }
        }
        
        // If not found self, leave it empty (which will cause error on elision)
    }
    
    // Process return type
    if (node.returnType) {
        // We temporarily store the elidedOutputLifetime in a class member or just capture it via a lambda?
        // Let's use a member variable or just traverse manually to keep it simple.
        // Wait, since we are visiting ReferenceTypeNode recursively, we need to pass this down.
        // We can use a member `std::string currentElidedOutputLifetime;`
        // But let's avoid adding more members.
        
        struct ReturnVisitor : public TypeVisitor {
            DiagnosticEngine& diag;
            std::string defaultLifetime;
            SourceLocation funcLoc;
            std::vector<std::string>& newLifetimes;

            ReturnVisitor(DiagnosticEngine& d, std::string defaultLt, SourceLocation loc, std::vector<std::string>& nl)
                : diag(d), defaultLifetime(defaultLt), funcLoc(loc), newLifetimes(nl) {}

            void visit(BuiltinTypeNode&) override {}
            void visit(NamedTypeNode& node) override {
                for (auto& arg : node.genericArgs) arg->accept(*this);
                for (auto& b : node.associatedBindings) b.type->accept(*this);
            }
            void visit(LifetimeNode&) override {}
            void visit(ReferenceTypeNode& node) override {
                if (!node.lifetime) {
                    if (defaultLifetime.empty()) {
                        diag.error(node.loc, "Cannot infer an appropriate lifetime for this reference. There are multiple input lifetimes and no 'self' parameter.");
                    } else {
                        auto lt = std::make_unique<LifetimeNode>();
                        lt->loc = node.loc;
                        lt->name = defaultLifetime;
                        node.lifetime = std::move(lt);
                    }
                } else {
                    node.lifetime->accept(*this);
                }
                if (node.inner) node.inner->accept(*this);
            }
            void visit(PointerTypeNode& node) override { if (node.inner) node.inner->accept(*this); }
            void visit(ArrayTypeNode& node) override { if (node.elementType) node.elementType->accept(*this); }
            void visit(TupleTypeNode& node) override { for (auto& t : node.elements) t->accept(*this); }
            void visit(FunctionTypeNode& node) override {
                // Function pointers have their own elision scope, skip for MVP
            }
            void visit(NeverTypeNode&) override {}
            void visit(TraitObjectTypeNode& node) override { if (node.trait) node.trait->accept(*this); }
        };
        
        ReturnVisitor rv(diag, elidedOutputLifetime, node.loc, declaredLifetimes);
        node.returnType->accept(rv);
    }
    
    // Inject newly generated anonymous lifetimes into function's generic parameters
    for (const auto* ltNode : currentInputLifetimes) {
        std::string ltName(ltNode->name);
        bool found = false;
        for (const auto& gp : node.genericParams) {
            if (gp.name == ltName) {
                found = true;
                break;
            }
        }
        if (!found) {
            GenericParamNode param;
            param.loc = node.loc;
            param.name = ltNode->name;
            param.kind = GenericParamKind::Lifetime;
            // Prepend to match Rust's typical ordering (lifetimes before types)
            node.genericParams.insert(node.genericParams.begin(), std::move(param));
        }
    }
    
    if (node.body) node.body->accept(*this);
}

void LifetimeElider::visit(ParamDeclNode& node) {
    if (node.type) node.type->accept(static_cast<TypeVisitor&>(*this));
}

void LifetimeElider::visit(StructDeclNode& node) {
    for (auto& f : node.fields) f->accept(*this);
}
void LifetimeElider::visit(EnumDeclNode& node) {
    for (auto& v : node.variants) v->accept(*this);
}
void LifetimeElider::visit(TraitDeclNode& node) {
    for (auto& m : node.methods) m->accept(*this);
}
void LifetimeElider::visit(ImplDeclNode& node) {
    for (auto& m : node.methods) m->accept(*this);
}
void LifetimeElider::visit(ModDeclNode& node) {
    for (auto& i : node.decls) i->accept(*this);
}
void LifetimeElider::visit(ExternDeclNode& node) {
    if (node.func) node.func->accept(*this);
}

void LifetimeElider::visit(NamedTypeNode& node) {
    for (auto& arg : node.genericArgs) arg->accept(*this);
    for (auto& b : node.associatedBindings) b.type->accept(*this);
}
void LifetimeElider::visit(LifetimeNode& node) {
    if (inInputParameters) {
        // Collect this explicit lifetime
        // To avoid duplicates, we can just push it (the rules say if there are multiple *distinct* lifetimes)
        // Wait, if it's the same named lifetime, we should count it as one.
        bool found = false;
        for (auto* l : currentInputLifetimes) {
            if (l->name == node.name) {
                found = true;
                break;
            }
        }
        if (!found) {
            currentInputLifetimes.push_back(&node);
        }
    }
}
void LifetimeElider::visit(ReferenceTypeNode& node) {
    if (inInputParameters) {
        if (!node.lifetime) {
            // Elide input lifetime
            std::string newLtName = generateAnonymousLifetime();
            
            // We need to inject this into the enclosing function's generic params later.
            // But we don't have direct access here. It's handled by `declaredLifetimes` in `visit(FunctionDeclNode)`.
            // Wait, we need to pass the newly generated string somewhere so the function knows.
            // Oh, we can just let the AST keep the string. We need to allocate it because string_view requires stable storage!
            // Wait, if we use dynamically allocated strings for the name, how is it managed?
            // Mellis' AST tokens are string_views backed by the source file.
            // Generated strings won't be in the source file! 
            // We need a stable storage for compiler-generated strings, like an `ASTContext` or `StringPool`.
            // But right now we don't have a `StringPool`. Let's just create a static vector for now to hold generated strings and return their string_views.
            static std::vector<std::unique_ptr<std::string>> generatedStrings;
            auto s = std::make_unique<std::string>(newLtName);
            std::string_view sv = *s;
            generatedStrings.push_back(std::move(s));

            auto lt = std::make_unique<LifetimeNode>();
            lt->loc = node.loc;
            lt->name = sv;
            node.lifetime = std::move(lt);
        }
        node.lifetime->accept(static_cast<TypeVisitor&>(*this));
    }
    if (node.inner) node.inner->accept(static_cast<TypeVisitor&>(*this));
}
void LifetimeElider::visit(PointerTypeNode& node) {
    if (node.inner) node.inner->accept(*this);
}
void LifetimeElider::visit(ArrayTypeNode& node) {
    if (node.elementType) node.elementType->accept(*this);
}
void LifetimeElider::visit(TupleTypeNode& node) {
    for (auto& t : node.elements) t->accept(*this);
}
void LifetimeElider::visit(FunctionTypeNode& node) {
    for (auto& p : node.params) p->accept(*this);
    if (node.returnType) node.returnType->accept(*this);
}
void LifetimeElider::visit(TraitObjectTypeNode& node) {
    if (node.trait) node.trait->accept(*this);
}

} // namespace fl
