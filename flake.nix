{
  description = "LambdaDX";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    lnmai-core-rs = {
      url = "git+ssh://git@github.com/pingfanH/lnmai-core-rs?ref=master";
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
          gmp
          libGL
          libuv
          libxkbcommon
          libx11
          libxcursor
          libxi
          libxinerama
          libxrandr
          udev
          wayland
          vulkan-loader
          zlib
        ];
        devLibs = map pkgs.lib.getDev libs;
        libraryPath = pkgs.lib.makeLibraryPath libs;
        pkgConfigPath = "${pkgs.lib.makeSearchPath "lib/pkgconfig" devLibs}:${pkgs.lib.makeSearchPath "share/pkgconfig" devLibs}";
        lnmaiCoreArtifacts = lnmai-core.packages.${system}.ffi-artifacts;
        stagedSource = pkgs.runCommand "lambdadx-source" {
          nativeBuildInputs = [ pkgs.rsync ];
        } ''
          mkdir -p \
            "$out/lnmai-core-rs" \
            "$out/maisimai" \
            "$out/lnmai-core-rs/lnmai-core-ffi" \
            "$out/lnmai-core-rs/lnmai-core-ffi/lnmai-core"

          rsync -a --chmod=Du+w,Dgo+rx,Fu+w,Fgo+r \
            --exclude .git/ \
            --exclude target/ \
            --exclude result \
            --exclude lnmai-core-rs/ \
            --exclude maisimai/ \
            ${self}/ "$out/"
          rsync -a --chmod=Du+w,Dgo+rx,Fu+w,Fgo+r \
            --exclude .git/ \
            --exclude target/ \
            --exclude result \
            --exclude lnmai-core-ffi/ \
            ${lnmai-core-rs}/. "$out/lnmai-core-rs/"
          rsync -a --chmod=Du+w,Dgo+rx,Fu+w,Fgo+r \
            --exclude .git/ \
            --exclude target/ \
            --exclude result \
            ${maisimai}/. "$out/maisimai/"
          rsync -a --chmod=Du+w,Dgo+rx,Fu+w,Fgo+r \
            --exclude .git/ \
            --exclude target/ \
            --exclude result \
            --exclude lnmai-core/ \
            ${lnmai-core-ffi}/. "$out/lnmai-core-rs/lnmai-core-ffi/"
          rsync -a --chmod=Du+w,Dgo+rx,Fu+w,Fgo+r \
            --exclude .git/ \
            --exclude target/ \
            --exclude result \
            --exclude .lake/ \
            ${lnmai-core}/. "$out/lnmai-core-rs/lnmai-core-ffi/lnmai-core/"

          chmod -R u+w "$out"
        '';
        lambdaDxPlayer = pkgs.rustPlatform.buildRustPackage {
          pname = "lambda_dx";
          version = "0.1.0";
          src = stagedSource;

          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "--bin" "lambda_dx_player" ];
          cargoInstallFlags = [ "--bin" "lambda_dx_player" ];

          nativeBuildInputs = with pkgs; [
            makeWrapper
            pkg-config
          ];
          buildInputs = libs ++ devLibs;

          LNMAI_CORE_ARTIFACTS = "${lnmaiCoreArtifacts}";
          LIBRARY_PATH = libraryPath;
          LD_LIBRARY_PATH = libraryPath;
          PKG_CONFIG_PATH = pkgConfigPath;

          doCheck = false;

          postFixup = ''
            wrapProgram "$out/bin/lambda_dx_player" \
              --prefix LD_LIBRARY_PATH : "${libraryPath}"
          '';
        };
        commonEnv = ''
          export RUST_BACKTRACE=1
          export LNMAI_CORE_ARTIFACTS="${lnmaiCoreArtifacts}"
          export LD_LIBRARY_PATH="${libraryPath}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          export LIBRARY_PATH="${libraryPath}''${LIBRARY_PATH:+:$LIBRARY_PATH}"
          export PKG_CONFIG_PATH="${pkgConfigPath}''${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
        '';
      in {
        packages.default = lambdaDxPlayer;
        packages.player = lambdaDxPlayer;

        apps.default = {
          type = "app";
          program = "${lambdaDxPlayer}/bin/lambda_dx_player";
        };

        apps.player = {
          type = "app";
          program = "${lambdaDxPlayer}/bin/lambda_dx_player";
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            binutils
            cargo
            git
            pkg-config
            rsync
            rustc
            rustfmt
            stdenv.cc
          ];

          buildInputs = libs ++ devLibs;

          LNMAI_CORE_ARTIFACTS = "${lnmaiCoreArtifacts}";
          LD_LIBRARY_PATH = libraryPath;
          LIBRARY_PATH = libraryPath;
          PKG_CONFIG_PATH = pkgConfigPath;

          shellHook = ''
            ${commonEnv}
            export CARGO_TARGET_DIR="$PWD/target/nix"
          '';
        };
      });
}
