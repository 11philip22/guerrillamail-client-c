cmake_minimum_required(VERSION 3.20)

project(guerrillamail_client_c_demo C)

find_program(CARGO_EXECUTABLE cargo REQUIRED)

set(
    GUERRILLAMAIL_CLIENT_C_ROOT
    "${CMAKE_CURRENT_SOURCE_DIR}/.."
    CACHE PATH
    "Path to the guerrillamail-client-c repository root"
)
get_filename_component(
    GUERRILLAMAIL_CLIENT_C_ROOT
    "${GUERRILLAMAIL_CLIENT_C_ROOT}"
    ABSOLUTE
)

set(
    GUERRILLAMAIL_CLIENT_C_PROFILE
    "Debug"
    CACHE STRING
    "Cargo profile used to build the Rust library (Debug or Release)"
)
set_property(
    CACHE GUERRILLAMAIL_CLIENT_C_PROFILE
    PROPERTY STRINGS Debug Release
)

if(NOT GUERRILLAMAIL_CLIENT_C_PROFILE STREQUAL "Debug"
   AND NOT GUERRILLAMAIL_CLIENT_C_PROFILE STREQUAL "Release")
    message(
        FATAL_ERROR
        "GUERRILLAMAIL_CLIENT_C_PROFILE must be Debug or Release"
    )
endif()

set(_cargo_args build)
if(GUERRILLAMAIL_CLIENT_C_PROFILE STREQUAL "Release")
    list(APPEND _cargo_args --release)
    set(_cargo_target_dir "${GUERRILLAMAIL_CLIENT_C_ROOT}/target/release")
else()
    set(_cargo_target_dir "${GUERRILLAMAIL_CLIENT_C_ROOT}/target/debug")
endif()

add_custom_target(
    guerrillamail_client_c_rust
    COMMAND "${CARGO_EXECUTABLE}" ${_cargo_args}
    WORKING_DIRECTORY "${GUERRILLAMAIL_CLIENT_C_ROOT}"
    COMMENT "Building the Rust guerrillamail-client-c library with Cargo"
    VERBATIM
)

if(WIN32)
    set(_shared_lib "${_cargo_target_dir}/guerrillamail_client_c.dll")
    if(MSVC)
        set(_link_lib "${_cargo_target_dir}/guerrillamail_client_c.dll.lib")
    else()
        set(_link_lib "${_cargo_target_dir}/libguerrillamail_client_c.dll.a")
    endif()
elseif(APPLE)
    set(_shared_lib "${_cargo_target_dir}/libguerrillamail_client_c.dylib")
    set(_link_lib "${_shared_lib}")
else()
    set(_shared_lib "${_cargo_target_dir}/libguerrillamail_client_c.so")
    set(_link_lib "${_shared_lib}")
endif()

add_library(guerrillamail_client_c SHARED IMPORTED GLOBAL)
if(WIN32)
    set_target_properties(
        guerrillamail_client_c
        PROPERTIES
        IMPORTED_LOCATION "${_shared_lib}"
        IMPORTED_IMPLIB "${_link_lib}"
    )
else()
    set_target_properties(
        guerrillamail_client_c
        PROPERTIES
        IMPORTED_LOCATION "${_shared_lib}"
    )
endif()

add_executable(demo demo.c)
add_dependencies(demo guerrillamail_client_c_rust)
target_include_directories(
    demo
    PRIVATE
    "${GUERRILLAMAIL_CLIENT_C_ROOT}/include"
)
target_link_libraries(demo PRIVATE guerrillamail_client_c)

if(APPLE)
    set_target_properties(demo PROPERTIES BUILD_RPATH "@loader_path")
elseif(UNIX)
    set_target_properties(demo PROPERTIES BUILD_RPATH "$ORIGIN")
endif()

add_custom_command(
    TARGET demo
    POST_BUILD
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "${_shared_lib}"
            "$<TARGET_FILE_DIR:demo>"
    COMMENT "Copying the Rust runtime library next to demo"
)
