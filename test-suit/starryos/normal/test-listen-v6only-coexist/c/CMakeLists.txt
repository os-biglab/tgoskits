cmake_minimum_required(VERSION 3.20)
project(test-listen-v6only-coexist C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

add_executable(test-listen-v6only-coexist src/main.c)
target_include_directories(test-listen-v6only-coexist PRIVATE src)
target_compile_options(test-listen-v6only-coexist PRIVATE -Wall -Wextra)

install(TARGETS test-listen-v6only-coexist RUNTIME DESTINATION usr/bin)
