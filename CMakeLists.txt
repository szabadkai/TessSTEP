cmake_minimum_required(VERSION 3.20)
project(TessSTEP VERSION 1.1.0 LANGUAGES C CXX)
include(GNUInstallDirs)
include(CMakePackageConfigHelpers)

# The public package ships the ABI library shared (TessSTEP::TessSTEP) and static
# (TessSTEP::TessSTEP_static). No Rust toolchain is needed by installed-package
# consumers. Cargo is used only to build from source.
find_program(CARGO_EXECUTABLE cargo REQUIRED)
find_program(RUSTC_EXECUTABLE rustc REQUIRED)
set(TESSSTEP_CARGO_TARGET "" CACHE STRING "Optional native Rust target triple")
set(TESSSTEP_LIBRARY_DIR "" CACHE PATH "Use an already-built release cdylib/staticlib directory")
if(WIN32)
    set(TS_LIBRARY tessstep_capi.dll)
    set(TS_IMPLIB tessstep_capi.dll.lib)
    set(TS_STATIC_LIBRARY tessstep_capi.lib)
elseif(APPLE)
    set(TS_LIBRARY libtessstep_capi.dylib)
    set(TS_STATIC_LIBRARY libtessstep_capi.a)
else()
    set(TS_LIBRARY libtessstep_capi.so)
    set(TS_STATIC_LIBRARY libtessstep_capi.a)
endif()
if(TESSSTEP_LIBRARY_DIR)
    set(TS_BUILD_DIR "${TESSSTEP_LIBRARY_DIR}")
else()
    set(TS_TARGET_ARGS)
    set(TS_BUILD_DIR "${CMAKE_CURRENT_BINARY_DIR}/cargo")
    if(TESSSTEP_CARGO_TARGET)
        list(APPEND TS_TARGET_ARGS --target "${TESSSTEP_CARGO_TARGET}")
        string(APPEND TS_BUILD_DIR "/${TESSSTEP_CARGO_TARGET}")
    endif()
    string(APPEND TS_BUILD_DIR "/release")
    set(TS_BYPRODUCTS "${TS_BUILD_DIR}/${TS_LIBRARY}" "${TS_BUILD_DIR}/${TS_STATIC_LIBRARY}")
    if(WIN32)
        list(APPEND TS_BYPRODUCTS "${TS_BUILD_DIR}/${TS_IMPLIB}")
    endif()
    add_custom_target(tessstep_cargo ALL
        COMMAND "${CARGO_EXECUTABLE}" build --manifest-path "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
            --package tessstep-capi --release --locked
            --target-dir "${CMAKE_CURRENT_BINARY_DIR}/cargo" ${TS_TARGET_ARGS}
        BYPRODUCTS ${TS_BYPRODUCTS}
        WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
        VERBATIM)
endif()

# The static library embeds the Rust standard library. Its system link dependencies
# are what rustc reports for the target; they become the static target's link
# interface, so consumers never need to know Rust linkage. An empty crate reports
# exactly the standard library's, and tessstep-capi adds no native dependency.
set(TS_PROBE_DIR "${CMAKE_CURRENT_BINARY_DIR}/native-static-libs")
file(MAKE_DIRECTORY "${TS_PROBE_DIR}")
file(WRITE "${TS_PROBE_DIR}/probe.rs" "")
set(TS_PROBE_TARGET)
if(TESSSTEP_CARGO_TARGET)
    set(TS_PROBE_TARGET --target "${TESSSTEP_CARGO_TARGET}")
endif()
execute_process(
    COMMAND "${RUSTC_EXECUTABLE}" --crate-type staticlib --crate-name tessstep_probe
        --print native-static-libs ${TS_PROBE_TARGET} --out-dir "${TS_PROBE_DIR}" "${TS_PROBE_DIR}/probe.rs"
    RESULT_VARIABLE TS_PROBE_RESULT
    OUTPUT_VARIABLE TS_PROBE_OUTPUT
    ERROR_VARIABLE TS_PROBE_OUTPUT)
string(REGEX MATCH "native-static-libs: ([^\r\n]*)" TS_PROBE_LINE "${TS_PROBE_OUTPUT}")
if(NOT TS_PROBE_RESULT EQUAL 0 OR NOT TS_PROBE_LINE)
    message(FATAL_ERROR "Cannot determine the Rust static library dependencies:\n${TS_PROBE_OUTPUT}")
endif()
separate_arguments(TS_NATIVE UNIX_COMMAND "${CMAKE_MATCH_1}")
set(TS_STATIC_DEPENDENCIES)
set(TS_FRAMEWORK OFF)
foreach(item IN LISTS TS_NATIVE)
    if(TS_FRAMEWORK)
        list(APPEND TS_STATIC_DEPENDENCIES "-framework ${item}")
        set(TS_FRAMEWORK OFF)
    elseif(item STREQUAL "-framework")
        set(TS_FRAMEWORK ON)
    elseif(item MATCHES "^-l(.+)$")
        # The Apple compiler driver always links libSystem; naming it again warns.
        if(NOT (APPLE AND CMAKE_MATCH_1 STREQUAL "System"))
            list(APPEND TS_STATIC_DEPENDENCIES "${CMAKE_MATCH_1}")
        endif()
    elseif(NOT item MATCHES "^/")
        # MSVC /defaultlib: directives are already embedded in the Rust objects.
        list(APPEND TS_STATIC_DEPENDENCIES "${item}")
    endif()
endforeach()
message(STATUS "TessSTEP static link dependencies: ${TS_STATIC_DEPENDENCIES}")

# A shared Rust cdylib carries its native dependencies; consumers link only it.
if(WIN32)
    install(FILES "${TS_BUILD_DIR}/${TS_LIBRARY}" DESTINATION "${CMAKE_INSTALL_BINDIR}")
    install(FILES "${TS_BUILD_DIR}/${TS_IMPLIB}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
else()
    install(FILES "${TS_BUILD_DIR}/${TS_LIBRARY}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
endif()
install(FILES "${TS_BUILD_DIR}/${TS_STATIC_LIBRARY}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
install(DIRECTORY include/tessstep DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}")
configure_package_config_file(cmake/TessSTEPConfig.cmake.in
    "${CMAKE_CURRENT_BINARY_DIR}/TessSTEPConfig.cmake"
    INSTALL_DESTINATION "${CMAKE_INSTALL_LIBDIR}/cmake/TessSTEP"
    PATH_VARS CMAKE_INSTALL_BINDIR CMAKE_INSTALL_LIBDIR CMAKE_INSTALL_INCLUDEDIR)
write_basic_package_version_file("${CMAKE_CURRENT_BINARY_DIR}/TessSTEPConfigVersion.cmake"
    VERSION "${PROJECT_VERSION}" COMPATIBILITY SameMajorVersion)
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/TessSTEPConfig.cmake"
    "${CMAKE_CURRENT_BINARY_DIR}/TessSTEPConfigVersion.cmake"
    DESTINATION "${CMAKE_INSTALL_LIBDIR}/cmake/TessSTEP")
