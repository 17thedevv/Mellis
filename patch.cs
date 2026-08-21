using System;
using System.IO;
using System.Text.RegularExpressions;

string text = File.ReadAllText("compiler/src/MiddleEnd/TypeChecker.cpp");
string pattern = @"constraints\.push_back\(Constraint\(ConstraintKind::Equality, instTy, field\.value->inferredType, "field type mismatch", node\.loc\)\);";
string replacement = @"std::cerr << "[DEBUG StructInitExpr] fId=" << fId << " fTy=" << (fTy ? std::to_string((int)fTy->getKind()) : "null") << " instTy=" << (instTy ? std::to_string((int)instTy->getKind()) : "null") << " field.value->inferredType=" << (field.value->inferredType ? std::to_string((int)field.value->inferredType->getKind()) : "null") << "\n";
                                  constraints.push_back(Constraint(ConstraintKind::Equality, instTy, field.value->inferredType, "field type mismatch", node.loc));";
text = Regex.Replace(text, pattern, replacement);
File.WriteAllText("compiler/src/MiddleEnd/TypeChecker.cpp", text);
