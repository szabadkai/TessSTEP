cmake_minimum_required(VERSION 3.20)
project(TessSTEP VERSION 0.1.0 LANGUAGES C CXX)
include(GNUInstallDirs)
include(CMakePackageConfigHelpers)

# The initial public package ships one shared ABI library. No Rust toolchain is
# needed by installed-package consumers. Cargo is used only to build from source.
find_program(CARGO_EXECUTABLE cargo REQUIRED)
set(TESSSTEP_CARGO_TARGET "" CACHE STRING "Optional native Rust target triple")
set(TESSSTEP_LIBRARY_DIR "" CACHE PATH "Use an already-built release cdylib directory")
if(WIN32)
    set(TS_LIBRARY tessstep_capi.dll)
    set(TS_IMPLIB tessstep_capi.dll.lib)
elseif(APPLE)
    set(TS_LIBRARY libtessstep_capi.dylib)
else()
    set(TS_LIBRARY libtessstep_capi.so)
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
    set(TS_BYPRODUCTS "${TS_BUILD_DIR}/${TS_LIBRARY}")
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
# A shared Rust cdylib carries its native dependencies; consumers link only it.
if(WIN32)
    install(FILES "${TS_BUILD_DIR}/${TS_LIBRARY}" DESTINATION "${CMAKE_INSTALL_BINDIR}")
    install(FILES "${TS_BUILD_DIR}/${TS_IMPLIB}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
else()
    install(FILES "${TS_BUILD_DIR}/${TS_LIBRARY}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
endif()
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
