{ pkgs ? import <nixpkgs> {} }:

let
  libs = with pkgs; [
    alsa-lib
    libGL
    libxkbcommon
    udev
    wayland
    vulkan-loader
    xorg.libX11
    xorg.libXcursor
    xorg.libXi
    xorg.libXinerama
    xorg.libXrandr
  ];
in
pkgs.mkShell {
  packages = with pkgs; [
    cargo
    elan
    git
    pkg-config
    rustc
    rustfmt
  ];

  buildInputs = libs;

  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath libs;

  shellHook = ''
    export RUST_BACKTRACE=1

    if command -v lean >/dev/null 2>&1; then
      LEAN_PREFIX="$(lean --print-prefix 2>/dev/null || true)"
      if [ -n "$LEAN_PREFIX" ] && [ -d "$LEAN_PREFIX/lib" ]; then
        export LD_LIBRARY_PATH="$LEAN_PREFIX/lib''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
      fi
    fi
  '';
}
