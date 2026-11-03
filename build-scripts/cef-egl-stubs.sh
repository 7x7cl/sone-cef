#!/bin/sh
# Workaround for the Tauri 3 alpha bundler: its hardcoded CEF file list
# includes libEGL.so and libGLESv2.so, but the CEF 154 distribution no
# longer ships them, so the bundle step fails with ENOENT.
#
# We generate a minimal (valid, stripped-safe) stub ELF in the downloaded
# CEF dist dir so the bundler has something to copy. The stub has no EGL
# symbols, so at runtime Chromium's ANGLE load fails on the bundled lib and
# falls back to the system EGL driver (e.g. NVIDIA's libEGL).
#
# Runs as `beforeBundleCommand`; no-op on non-Linux or when the dist dir
# is not present yet.

[ "$(uname -s)" = "Linux" ] || exit 0

dir=$(find "${XDG_CACHE_HOME:-$HOME/.cache}/tauri-cef" -mindepth 2 -maxdepth 2 -type d -name 'cef_linux_*64' 2>/dev/null | head -n 1)
[ -d "$dir" ] || exit 0

for lib in libEGL.so libGLESv2.so; do
    if ! file "$dir/$lib" 2>/dev/null | grep -q 'ELF.*shared object'; then
        printf 'void __cef_stub(void){}\n' | cc -shared -fPIC -x c - -o "$dir/$lib" 2>/dev/null || :
    fi
done
