# libdave interoperability tests

These tests check the cordial DAVE session against [libdave](https://github.com/discord/libdave), the C++ DAVE library of Discord. Each test makes a call with cordial members and libdave members. The test gateway of `cordial-voice` (feature `test-gateway`) is the MLS external sender.

The crate is not published. The workspace forbids unsafe code, so this crate has its own lints for the FFI calls.

## What the tests check

| Test | Check |
|---|---|
| `libdave_joins_a_group_that_cordial_commits` | libdave accepts a cordial commit and Welcome. Frames pass both ways. |
| `cordial_joins_a_group_that_libdave_commits` | cordial accepts a libdave Welcome. Frames pass both ways. |
| `mixed_group_commits_from_both_sides_and_removes_a_member` | Three members. Each library processes a commit of the other. A removed member cannot decrypt new frames. |
| `frames_out_of_order_pass_and_replays_fail` | Frames in reverse order decrypt. Both libraries refuse a replayed frame. |
| `silence_frames_pass_both_ways` | Both libraries handle the Opus silence frame. |

## Build libdave

You need Git, CMake, and a C++17 compiler. On Windows, use Visual Studio with the C++ tools.

1. Clone libdave with its vcpkg submodule:

   ```sh
   git clone --recurse-submodules https://github.com/discord/libdave.git
   cd libdave/cpp
   ```

2. Bootstrap vcpkg: `./vcpkg/bootstrap-vcpkg.sh` (Windows: `vcpkg\bootstrap-vcpkg.bat`).
3. Build the shared library with OpenSSL 3:

   ```sh
   cmake -B build -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=ON \
     -DVCPKG_MANIFEST_DIR=vcpkg-alts/openssl_3 \
     -DCMAKE_TOOLCHAIN_FILE=vcpkg/scripts/buildsystems/vcpkg.cmake
   cmake --build build --config Release --target libdave
   ```

   On Windows, also give `-DVCPKG_TARGET_TRIPLET=x64-windows-static -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded`.

### Windows problems

- vcpkg fails with "Filename too long": map the folder to a drive letter with `subst`, and build from that drive.
- vcpkg fails with "Unable to find a valid Visual Studio instance": the vcpkg version of the submodule does not know your Visual Studio. Check out a newer vcpkg release tag in `cpp/vcpkg`, and run the bootstrap again.
- Do not build the libdave unit tests (`-DTESTING=ON`) with a shared library on Windows. The DLL exports only the C API, so the unit tests do not link. Use a separate static build for them.

## Run the tests

Set `LIBDAVE_PATH` to the shared library, and run the ignored tests:

```sh
LIBDAVE_PATH=/path/to/libdave.so cargo test -p libdave-interop -- --ignored
```

On Windows, the library is `build\Release\libdave.dll`.

A libdave warning "Decrypt failed, no valid cryptor found" is expected. It comes from the replay check.
