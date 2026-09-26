{
  description = "LambdaDX";

  nixConfig = {
    extra-substituters = [ "https://lnmai-core.cachix.org" ];
    extra-trusted-public-keys = [
      "lnmai-core.cachix.org-1:rYcjvGbYnD1X9NWUExTn2dly2tFFzuamDEj02rJG7F8="
    ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    lnmai-core-ffi = {
      url = "github:pingfanH/lnmai-core-ffi?ref=master";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
      inputs.lnmai-core.follows = "lnmai-core";
    };
    lnmai-core = {
      url = "github:Neuron-Group/lnmai-core?rev=a30b0fc63ec5abf60393b979791a199266614bd1";
      inputs.nixpkgs.url = "github:NixOS/nixpkgs/567a49d1913ce81ac6e9582e3553dd90a955875f";
    };
    maisimai = {
      url = "github:pingfanH/maisimai-rs?ref=master";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
  };

  outputs = { self, nixpkgs, flake-utils, lnmai-core-ffi, lnmai-core, maisimai }:
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
        cjkFont = pkgs.noto-fonts-cjk-sans;
        cjkFontPath = "${cjkFont}/share/fonts/opentype/noto-cjk/NotoSansCJK-VF.otf.ttc";
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
          cargoBuildFlags = [ "--bin" "lambda_dx_player_ui" ];
          cargoInstallFlags = [ "--bin" "lambda_dx_player_ui" ];

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
            mkdir -p "$out/share/lambda_dx"
            cp -r "$src/assets" "$out/share/lambda_dx/assets"
            wrapProgram "$out/bin/lambda_dx_player_ui" \
              --prefix LD_LIBRARY_PATH : "${libraryPath}" \
              --set MAI2_ASSET_DIR "$out/share/lambda_dx/assets" \
              --set MAI2_FONT_PATH "${cjkFontPath}"
          '';
        };
        commonEnv = ''
          export RUST_BACKTRACE=1
          export LNMAI_CORE_ARTIFACTS="${lnmaiCoreArtifacts}"
          export MAI2_FONT_PATH="${cjkFontPath}"
          export LD_LIBRARY_PATH="${libraryPath}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          export LIBRARY_PATH="${libraryPath}''${LIBRARY_PATH:+:$LIBRARY_PATH}"
          export PKG_CONFIG_PATH="${pkgConfigPath}''${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
        '';
      in {
        packages.default = lambdaDxPlayer;
        packages.player = lambdaDxPlayer;

        apps.default = {
          type = "app";
          program = "${lambdaDxPlayer}/bin/lambda_dx_player_ui";
        };

        apps.player = {
          type = "app";
          program = "${lambdaDxPlayer}/bin/lambda_dx_player_ui";
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
