cmake_minimum_required(VERSION 3.22.1)
project("pocketbridge_native")

add_library(
    pocketbridge_native
    SHARED
    chunk_transform_native.cpp
    sha256.cpp
)

target_include_directories(pocketbridge_native PRIVATE .)

target_compile_options(
    pocketbridge_native
    PRIVATE
    -O3
    -flto
    -fvisibility=hidden
)

find_library(log-lib log)

target_link_libraries(
    pocketbridge_native
    ${log-lib}
)
