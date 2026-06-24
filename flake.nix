{
  description = "LambdaDX";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    lnmai-core-rs = {
      url = "git+ssh://git@github.com/pingfanH/lnmai-core-rs?ref=main";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
    lnmai-core-ffi = {
      url = "git+ssh://git@github.com/pingfanH/lnmai-core-ffi?ref=master";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
    lnmai-core = {
      url = "git+ssh://git@github.com/Neuron-Group/lnmai-core?ref=main";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
    maisimai = {
      url = "git+ssh://git@github.com/pingfanH/maisimai-rs?ref=master";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
  };

  outputs = { self, nixpkgs, flake-utils, lnmai-core-rs, lnmai-core-ffi, lnmai-core, maisimai }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        lib = pkgs.lib;
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
        cargoBin = "${pkgs.cargo}/bin/cargo";
        rustcBin = "${pkgs.rustc}/bin/rustc";
        ccBin = "${pkgs.stdenv.cc}/bin/cc";
        cxxBin = "${pkgs.stdenv.cc}/bin/c++";
        arBin = "${pkgs.binutils}/bin/ar";
        ranlibBin = "${pkgs.binutils}/bin/ranlib";
        basePath = pkgs.lib.makeBinPath [
          pkgs.bash
          pkgs.binutils
          pkgs.cargo
          pkgs.coreutils
          pkgs.elan
          pkgs.git
          pkgs.pkg-config
          pkgs.rsync
          pkgs.rustc
          pkgs.stdenv.cc
        ];
        commonEnv = ''
          export RUST_BACKTRACE=1
          export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath libs}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPath "lib/pkgconfig" devLibs}:${pkgs.lib.makeSearchPath "share/pkgconfig" devLibs}''${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
          export PATH="${basePath}:$PATH"
          export CARGO="${cargoBin}"
          export RUSTC="${rustcBin}"
          export CC="${ccBin}"
          export CXX="${cxxBin}"
          export AR="${arBin}"
          export RANLIB="${ranlibBin}"
        '';
        repoSetup = ''
          ${commonEnv}
          if [ -n "''${LAMBDA_DX_REPO_ROOT:-}" ]; then
            repo_root="$LAMBDA_DX_REPO_ROOT"
          else
            repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
          fi

          workspace_root="$repo_root/target/nix/workspace"
          source_root="$workspace_root/source"

          mkdir -p "$source_root"
          chmod -R u+w "$source_root" 2>/dev/null || true

          mkdir -p \
            "$source_root/lnmai-core-rs" \
            "$source_root/maisimai" \
            "$source_root/lnmai-core-rs/lnmai-core-ffi" \
            "$source_root/lnmai-core-rs/lnmai-core-ffi/lnmai-core"

          rsync -a --delete --chmod=Du+w,Dgo+rx,Fu+w,Fgo+r \
            --exclude lnmai-core-rs/ \
            --exclude maisimai/ \
            ${self}/ "$source_root/"
          rsync -a --delete --chmod=Du+w,Dgo+rx,Fu+w,Fgo+r ${lnmai-core-rs}/. "$source_root/lnmai-core-rs/"
          rsync -a --delete --chmod=Du+w,Dgo+rx,Fu+w,Fgo+r ${maisimai}/. "$source_root/maisimai/"
          rsync -a --delete --chmod=Du+w,Dgo+rx,Fu+w,Fgo+r --exclude lnmai-core/ ${lnmai-core-ffi}/. "$source_root/lnmai-core-rs/lnmai-core-ffi/"
          rsync -a --delete --chmod=Du+w,Dgo+rx,Fu+w,Fgo+r --exclude .lake/ ${lnmai-core}/. "$source_root/lnmai-core-rs/lnmai-core-ffi/lnmai-core/"
          chmod -R u+w "$source_root"

          cd "$source_root"
          export CARGO_TARGET_DIR="$repo_root/target/nix"
        '';
        lambdaDxPlayerApp = pkgs.writeShellApplication {
          name = "lambdadx-player";
          runtimeInputs = with pkgs; [
            bash
            binutils
            cargo
            coreutils
            elan
            git
            pkg-config
            rsync
            rustc
            stdenv.cc
          ];
          text = ''
            ${repoSetup}
            exec "$CARGO" run --bin lambda_dx_player -- "$@"
          '';
        };
        lambdaDxBuildApp = pkgs.writeShellApplication {
          name = "lambdadx-build";
          runtimeInputs = with pkgs; [
            bash
            binutils
            cargo
            coreutils
            elan
            git
            pkg-config
            rsync
            rustc
            stdenv.cc
          ];
          text = ''
            ${repoSetup}
            if [ "$#" -eq 0 ]; then
              set -- --bin lambda_dx_player
            fi
            exec "$CARGO" build "$@"
          '';
        };
      in {
        packages.default = lambdaDxBuildApp;
        packages.player = lambdaDxPlayerApp;
        packages.cli-build = lambdaDxBuildApp;

        apps.default = {
          type = "app";
          program = "${lambdaDxBuildApp}/bin/lambdadx-build";
        };

        apps.player = {
          type = "app";
          program = "${lambdaDxPlayerApp}/bin/lambdadx-player";
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            binutils
            cargo
            elan
            git
            pkg-config
            rsync
            rustc
            rustfmt
            stdenv.cc
          ];

          buildInputs = libs ++ devLibs;

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath libs;

          shellHook = ''
            ${commonEnv}
            export CARGO_TARGET_DIR="$PWD/target/nix"
          '';
        };
      });
}
