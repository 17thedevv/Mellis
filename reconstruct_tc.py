lines = []
with open('recovered_clean.txt', 'r', encoding='utf-8') as f:
    for line in f:
        line = line.strip('\n')
        idx = line.find(':')
        if idx != -1:
            lineno = int(line[:idx].strip())
            content = line[idx+1:]
            if content.startswith(' '):
                content = content[1:]
            lines.append((lineno, content))

with open(r'D:\fdlang\src\MiddleEnd\TypeChecker.cpp', 'w', encoding='utf-8') as out:
    for lineno, content in lines:
        if lineno == 1868:
            out.write('                                     }\n')
            out.write('                                 }\n')
            out.write('                                 const Type* expectedFnType = ctx.getFunctionType(std::move(callArgNames), {leftTy, rightTy}, c.right, false);\n')
            out.write('                                 newConstraints.push_back(Constraint(ConstraintKind::Equality, mInfo.type, expectedFnType, "", c.loc));\n')
            out.write('                                 changed = true;\n')
            out.write('                             }\n')
            out.write('                         }\n')
        out.write(content + '\n')
