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
        devLibs = map pkgs.lib.getDev libs;
        pipelineSetup = ''
          export RUST_BACKTRACE=1
          export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath libs}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPath "lib/pkgconfig" devLibs}:${pkgs.lib.makeSearchPath "share/pkgconfig" devLibs}''${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
          export PATH="${pkgs.lib.makeBinPath [ pkgs.bash pkgs.binutils pkgs.cargo pkgs.elan pkgs.git pkgs.pkg-config pkgs.rustc pkgs.stdenv.cc ]}:$PATH"
          export CARGO="${pkgs.cargo}/bin/cargo"
          export RUSTC="${pkgs.rustc}/bin/rustc"
          export CC="${pkgs.stdenv.cc}/bin/cc"
          export CXX="${pkgs.stdenv.cc}/bin/c++"
          export AR="${pkgs.binutils}/bin/ar"
          export RANLIB="${pkgs.binutils}/bin/ranlib"

          if command -v lean >/dev/null 2>&1; then
            LEAN_PREFIX="$(lean --print-prefix 2>/dev/null || true)"
            if [ -n "$LEAN_PREFIX" ] && [ -d "$LEAN_PREFIX/lib" ]; then
              export LD_LIBRARY_PATH="$LEAN_PREFIX/lib''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
            fi
          fi

          if ! command -v git >/dev/null 2>&1; then
            echo "error: git is required" >&2
            exit 1
          fi

          repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
          if [ -z "$repo_root" ]; then
            echo "error: run this command from inside the LambdaDX repository" >&2
            exit 1
          fi

          cd "$repo_root"
          export CARGO_TARGET_DIR="$repo_root/target/nix"

          echo "==> syncing submodules"
          git submodule sync --recursive
          git submodule update --init --recursive

          lean_project="$repo_root/lnmai-core-rs/lnmai-core-ffi/lnmai-core"
          lean_toolchain_file="$lean_project/lean-toolchain"

          if [ ! -f "$lean_toolchain_file" ]; then
            echo "error: missing Lean toolchain file at $lean_toolchain_file" >&2
            exit 1
          fi

          lean_toolchain="$(tr -d '[:space:]' < "$lean_toolchain_file")"
          if elan toolchain list | grep -Fq "$lean_toolchain"; then
            echo "==> using installed Lean toolchain $lean_toolchain"
          else
            echo "==> installing Lean toolchain $lean_toolchain"
            elan toolchain install "$lean_toolchain"
          fi

          echo "==> building Lean FFI"
          (
            cd "$lean_project"
            lake build LnmaiCore LnmaiCore.FFI
          )
        '';
        buildPipeline = pkgs.writeShellApplication {
          name = "lambdadx-build";
          runtimeInputs = with pkgs; [
            bash
            binutils
            cargo
            elan
            git
            pkg-config
            rustc
            stdenv.cc
          ];
          text = ''
            ${pipelineSetup}

            if [ "$#" -eq 0 ]; then
              set -- --bin lambda_dx_player
            fi

            echo "==> cargo build $*"
            exec "$CARGO" build "$@"
          '';
        };
        runPlayerPipeline = pkgs.writeShellApplication {
          name = "lambdadx-run-player";
          runtimeInputs = with pkgs; [
            bash
            binutils
            cargo
            elan
            git
            pkg-config
            rustc
            stdenv.cc
          ];
          text = ''
            ${pipelineSetup}

            echo "==> cargo run --bin lambda_dx_player $*"
            exec "$CARGO" run --bin lambda_dx_player -- "$@"
          '';
        };
      in {
        packages.default = buildPipeline;

        apps.default = {
          type = "app";
          program = "${buildPipeline}/bin/lambdadx-build";
        };

        apps.player = {
          type = "app";
          program = "${runPlayerPipeline}/bin/lambdadx-run-player";
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            binutils
            cargo
            elan
            git
            pkg-config
            rustc
            rustfmt
            stdenv.cc
          ];

          buildInputs = libs ++ devLibs;

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath libs;

          shellHook = ''
            export RUST_BACKTRACE=1
            export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPath "lib/pkgconfig" devLibs}:${pkgs.lib.makeSearchPath "share/pkgconfig" devLibs}''${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
            export PATH="${pkgs.lib.makeBinPath [ pkgs.binutils pkgs.cargo pkgs.elan pkgs.git pkgs.pkg-config pkgs.rustc pkgs.stdenv.cc ]}:$PATH"
            export CARGO="${pkgs.cargo}/bin/cargo"
            export RUSTC="${pkgs.rustc}/bin/rustc"
            export CC="${pkgs.stdenv.cc}/bin/cc"
            export CXX="${pkgs.stdenv.cc}/bin/c++"
            export AR="${pkgs.binutils}/bin/ar"
            export RANLIB="${pkgs.binutils}/bin/ranlib"
            export CARGO_TARGET_DIR="$PWD/target/nix"

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
