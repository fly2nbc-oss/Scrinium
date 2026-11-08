# All dependencies static, tesseract itself as DLL, dynamic CRT (/MD), release only.
set(VCPKG_TARGET_ARCHITECTURE x64)
set(VCPKG_CRT_LINKAGE dynamic)
set(VCPKG_LIBRARY_LINKAGE static)
set(VCPKG_BUILD_TYPE release)
if(PORT STREQUAL "tesseract")
    set(VCPKG_LIBRARY_LINKAGE dynamic)
endif()
