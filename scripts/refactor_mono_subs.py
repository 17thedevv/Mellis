import re

with open('src/MiddleEnd/MonomorphizationEngine.cpp', 'r', encoding='utf-8') as f:
    content = f.read()

# We need to replace the old substitution block with the new GenericSubstitution block.
# Old block pattern:
#     std::unordered_map<std::string, std::unique_ptr<TypeNode>> subs;
#     for (size_t i = 0; i < genericTemplate->genericParams.size() && i < genericArgs.size(); ++i) {
#         subs[std::string(genericTemplate->genericParams[i].name)] = typeToAST(genericArgs[i], symTable);
#     }
# 
#     SubstitutionVisitor visitor(std::move(subs));

def replace_subs(match):
    template_name = match.group(1)
    return f"""    GenericSubstitution subs;
    for (size_t i = 0; i < {template_name}->genericParams.size() && i < genericArgs.size(); ++i) {{
        auto astNode = typeToAST(genericArgs[i], symTable);
        if ({template_name}->genericParams[i].kind == GenericParamKind::Lifetime) {{
            if (auto* ltNode = dynamic_cast<LifetimeNode*>(astNode.get())) {{
                subs.lifetimeSubstitutions[std::string({template_name}->genericParams[i].name)] = std::unique_ptr<LifetimeNode>(static_cast<LifetimeNode*>(astNode.release()));
            }}
        }} else {{
            subs.typeSubstitutions[std::string({template_name}->genericParams[i].name)] = std::move(astNode);
        }}
    }}

    SubstitutionVisitor visitor(std::move(subs));"""

content = re.sub(
    r'std::unordered_map<std::string, std::unique_ptr<TypeNode>> subs;\s*for \(size_t i = 0; i < ([a-zA-Z0-9_]+)->genericParams\.size\(\) && i < genericArgs\.size\(\); \+\+i\) \{\s*subs\[std::string\(\1->genericParams\[i\]\.name\)\] = typeToAST\(genericArgs\[i\], symTable\);\s*\}\s*SubstitutionVisitor visitor\(std::move\(subs\)\);',
    replace_subs,
    content
)

# And for implSubs:
def replace_impl_subs(match):
    template_name = match.group(1)
    return f"""            GenericSubstitution implSubs;
            for (size_t i = 0; i < {template_name}->genericParams.size() && i < genericArgs.size(); ++i) {{
                auto astNode = typeToAST(genericArgs[i], symTable);
                if ({template_name}->genericParams[i].kind == GenericParamKind::Lifetime) {{
                    if (auto* ltNode = dynamic_cast<LifetimeNode*>(astNode.get())) {{
                        implSubs.lifetimeSubstitutions[std::string({template_name}->genericParams[i].name)] = std::unique_ptr<LifetimeNode>(static_cast<LifetimeNode*>(astNode.release()));
                    }}
                }} else {{
                    implSubs.typeSubstitutions[std::string({template_name}->genericParams[i].name)] = std::move(astNode);
                }}
            }}
            
            SubstitutionVisitor implVisitor(std::move(implSubs));"""

content = re.sub(
    r'std::unordered_map<std::string, std::unique_ptr<TypeNode>> implSubs;\s*for \(size_t i = 0; i < ([a-zA-Z0-9_]+)->genericParams\.size\(\) && i < genericArgs\.size\(\); \+\+i\) \{\s*implSubs\[std::string\(\1->genericParams\[i\]\.name\)\] = typeToAST\(genericArgs\[i\], symTable\);\s*\}\s*SubstitutionVisitor implVisitor\(std::move\(implSubs\)\);',
    replace_impl_subs,
    content
)

with open('src/MiddleEnd/MonomorphizationEngine.cpp', 'w', encoding='utf-8') as f:
    f.write(content)

print("Done")
