cmake_minimum_required(VERSION 3.20)
project(net-irq C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

add_executable(net-irq src/main.c)
target_compile_options(net-irq PRIVATE -Wall -Wextra)

install(TARGETS net-irq RUNTIME DESTINATION usr/bin)
