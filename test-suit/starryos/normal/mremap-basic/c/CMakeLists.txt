cmake_minimum_required(VERSION 3.20)
project(mremap-basic C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

add_executable(mremap-basic src/main.c)
target_compile_options(mremap-basic PRIVATE -Wall -Wextra -Werror)

install(TARGETS mremap-basic RUNTIME DESTINATION usr/bin)
