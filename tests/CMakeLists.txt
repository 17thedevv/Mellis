



add_executable(run_test ../scratch/run_test.cpp)
target_link_libraries(run_test PRIVATE mellis_frontend mellis_ast mellis_basic)

add_executable(test_macro2 test_macro.cpp)
target_link_libraries(test_macro2 PRIVATE mellis_frontend mellis_ast mellis_basic)
