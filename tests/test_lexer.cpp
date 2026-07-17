#include <cassert>
#include <iostream>
#include "fdlang/FrontEnd/Lexer.h"

using namespace fl;

void testVariableDeclaration() {
    Lexer lexer("dec a = 5; fn myFunc() -> int { return a; }");
    
    Token t1 = lexer.nextToken();
    assert(t1.type == TokenType::KW_DEC && "Expected KW_DEC");
    assert(t1.text == "dec" && "Expected text 'dec'");
    
    Token t2 = lexer.nextToken();
    assert(t2.type == TokenType::IDENTIFIER && "Expected IDENTIFIER");
    assert(t2.text == "a" && "Expected text 'a'");
    
    Token t3 = lexer.nextToken();
    assert(t3.type == TokenType::EQUAL && "Expected EQUAL");
    
    Token t4 = lexer.nextToken();
    assert(t4.type == TokenType::INTEGER_LITERAL && "Expected INTEGER_LITERAL");
    assert(t4.text == "5" && "Expected text '5'");
    
    Token t5 = lexer.nextToken();
    assert(t5.type == TokenType::SEMI && "Expected SEMI");
    
    Token t6 = lexer.nextToken();
    assert(t6.type == TokenType::KW_FN && "Expected KW_FN");
    
    Token t7 = lexer.nextToken();
    assert(t7.type == TokenType::IDENTIFIER && "Expected IDENTIFIER");
    assert(t7.text == "myFunc" && "Expected 'myFunc'");
    
    Token t8 = lexer.nextToken();
    assert(t8.type == TokenType::L_PAREN && "Expected L_PAREN");
    
    Token t9 = lexer.nextToken();
    assert(t9.type == TokenType::R_PAREN && "Expected R_PAREN");
    
    Token t10 = lexer.nextToken();
    assert(t10.type == TokenType::ARROW && "Expected ARROW");
    
    Token t11 = lexer.nextToken();
    assert(t11.type == TokenType::IDENTIFIER && "Expected IDENTIFIER");
    assert(t11.text == "int" && "Expected 'int'");
    
    Token t12 = lexer.nextToken();
    assert(t12.type == TokenType::L_BRACE && "Expected L_BRACE");

    Token t13 = lexer.nextToken();
    assert(t13.type == TokenType::KW_RETURN && "Expected KW_RETURN");

    Token t14 = lexer.nextToken();
    assert(t14.type == TokenType::IDENTIFIER && "Expected IDENTIFIER");
    assert(t14.text == "a" && "Expected 'a'");

    Token t15 = lexer.nextToken();
    assert(t15.type == TokenType::SEMI && "Expected SEMI");

    Token t16 = lexer.nextToken();
    assert(t16.type == TokenType::R_BRACE && "Expected R_BRACE");

    Token t17 = lexer.nextToken();
    assert(t17.type == TokenType::END_OF_FILE && "Expected EOF");

    std::cout << "[OK] testVariableDeclaration passed!\n";
}

void testLiterals() {
    Lexer lexer("123 0x1A_B 0b1010_1010 0o777 3.14 1e-5 10u32 'a' '\\n' b'x' \"str\" r\"raw\\str\" b\"byte_str\" r#\"raw_hash\"#");
    
    Token t1 = lexer.nextToken();
    assert(t1.type == TokenType::INTEGER_LITERAL && t1.text == "123");
    
    Token t2 = lexer.nextToken();
    assert(t2.type == TokenType::INTEGER_LITERAL && t2.text == "0x1A_B");
    
    Token t3 = lexer.nextToken();
    assert(t3.type == TokenType::INTEGER_LITERAL && t3.text == "0b1010_1010");
    
    Token t4 = lexer.nextToken();
    assert(t4.type == TokenType::INTEGER_LITERAL && t4.text == "0o777");
    
    Token t5 = lexer.nextToken();
    assert(t5.type == TokenType::FLOAT_LITERAL && t5.text == "3.14");
    
    Token t6 = lexer.nextToken();
    assert(t6.type == TokenType::FLOAT_LITERAL && t6.text == "1e-5");

    Token t7 = lexer.nextToken();
    assert(t7.type == TokenType::INTEGER_LITERAL && t7.text == "10u32");

    Token t8 = lexer.nextToken();
    assert(t8.type == TokenType::CHAR_LITERAL && t8.text == "'a'");

    Token t9 = lexer.nextToken();
    assert(t9.type == TokenType::CHAR_LITERAL && t9.text == "'\\n'");

    Token t10 = lexer.nextToken();
    assert(t10.type == TokenType::BYTE_LITERAL && t10.text == "b'x'");

    Token t11 = lexer.nextToken();
    assert(t11.type == TokenType::STRING_LITERAL && t11.text == "\"str\"");

    Token t12 = lexer.nextToken();
    assert(t12.type == TokenType::RAW_STRING_LITERAL && t12.text == "r\"raw\\str\"");

    Token t13 = lexer.nextToken();
    assert(t13.type == TokenType::BYTE_STRING_LITERAL && t13.text == "b\"byte_str\"");

    Token t14 = lexer.nextToken();
    assert(t14.type == TokenType::RAW_STRING_LITERAL && t14.text == "r#\"raw_hash\"#");

    std::cout << "[OK] testLiterals passed!\n";
}

void testOperatorsAndPunctuation() {
    Lexer lexer("+= -= *= /= %= &= |= ^= <<= >>= .. ..= ... @[ @< :: -> && || ! ~ << >>");
    
    assert(lexer.nextToken().type == TokenType::PLUS_ASSIGN);
    assert(lexer.nextToken().type == TokenType::MINUS_ASSIGN);
    assert(lexer.nextToken().type == TokenType::STAR_ASSIGN);
    assert(lexer.nextToken().type == TokenType::SLASH_ASSIGN);
    assert(lexer.nextToken().type == TokenType::PERC_ASSIGN);
    assert(lexer.nextToken().type == TokenType::BIT_AND_ASSIGN);
    assert(lexer.nextToken().type == TokenType::BIT_OR_ASSIGN);
    assert(lexer.nextToken().type == TokenType::BIT_XOR_ASSIGN);
    assert(lexer.nextToken().type == TokenType::LSHIFT_ASSIGN);
    assert(lexer.nextToken().type == TokenType::RSHIFT_ASSIGN);
    
    assert(lexer.nextToken().type == TokenType::DOT_DOT);
    assert(lexer.nextToken().type == TokenType::DOT_DOT_EQ);
    assert(lexer.nextToken().type == TokenType::DOT_DOT_DOT);
    
    assert(lexer.nextToken().type == TokenType::AT_BRACKET);
    assert(lexer.nextToken().type == TokenType::GENERIC_START);
    assert(lexer.nextToken().type == TokenType::COLON_COLON);
    assert(lexer.nextToken().type == TokenType::ARROW);
    assert(lexer.nextToken().type == TokenType::LOGICAL_AND);
    assert(lexer.nextToken().type == TokenType::LOGICAL_OR);
    assert(lexer.nextToken().type == TokenType::BANG);
    assert(lexer.nextToken().type == TokenType::BIT_NOT);
    assert(lexer.nextToken().type == TokenType::LSHIFT);
    assert(lexer.nextToken().type == TokenType::RSHIFT);

    std::cout << "[OK] testOperatorsAndPunctuation passed!\n";
}

void testComments() {
    Lexer lexer("a // line comment\n b /* block \n comment */ c");
    
    assert(lexer.nextToken().text == "a");
    assert(lexer.nextToken().text == "b");
    assert(lexer.nextToken().text == "c");
    assert(lexer.nextToken().type == TokenType::END_OF_FILE);
    
    std::cout << "[OK] testComments passed!\n";
}

void testKeywords() {
    Lexer lexer("const type sizeof alignof await async comptime dyn self Self unsafe export");
    
    assert(lexer.nextToken().type == TokenType::KW_CONST);
    assert(lexer.nextToken().type == TokenType::KW_TYPE);
    assert(lexer.nextToken().type == TokenType::KW_SIZEOF);
    assert(lexer.nextToken().type == TokenType::KW_ALIGNOF);
    assert(lexer.nextToken().type == TokenType::KW_AWAIT);
    assert(lexer.nextToken().type == TokenType::KW_ASYNC);
    assert(lexer.nextToken().type == TokenType::KW_COMPTIME);
    assert(lexer.nextToken().type == TokenType::KW_DYN);
    assert(lexer.nextToken().type == TokenType::KW_SELF_VAL);
    assert(lexer.nextToken().type == TokenType::KW_SELF_TYP);
    assert(lexer.nextToken().type == TokenType::KW_UNSAFE);
    assert(lexer.nextToken().type == TokenType::KW_EXPORT);

    std::cout << "[OK] testKeywords passed!\n";
}

int main() {
    std::cout << "--- RUNNING FDLANG TESTS ---\n";
    
    testVariableDeclaration();
    testLiterals();
    testOperatorsAndPunctuation();
    testComments();
    testKeywords();
    
    std::cout << "--- ALL TESTS PASSED SUCCESSFULLY! ---\n";
    return 0;
}
