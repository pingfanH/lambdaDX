{
  description = "LambdaDX development shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        libs = with pkgs; [
          alsa-lib
          libGL
          libxkbcommon
          libx11
          libxcursor
          libxi
          libxinerama
          libxrandr
          udev
          wayland
          vulkan-loader
        ];
      in {
        devShells.default = pkgs.mkShell {
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
        };
      });
}
