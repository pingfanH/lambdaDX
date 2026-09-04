{
  description = "LambdaDX";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    lnmai-core = {
      url = "git+ssh://git@github.com/Neuron-Group/lnmai-core?ref=main";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
  };

  outputs = { self, nixpkgs, flake-utils, lnmai-core }:
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
        cjkFont = pkgs.wqy_microhei;
        cleanSource = lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            let
              rel = lib.removePrefix "${toString ./.}/" (toString path);
            in
              !(
                rel == ".git"
                || lib.hasPrefix ".git/" rel
                || rel == "target"
                || lib.hasPrefix "target/" rel
                || rel == "result"
                || lib.hasPrefix "result/" rel
                || rel == ".codegraph"
                || lib.hasPrefix ".codegraph/" rel
              );
        };
        lnmaiCoreArtifacts = lnmai-core.packages.${system}.ffi-artifacts;
        lambdaDxPlayer = pkgs.rustPlatform.buildRustPackage {
          pname = "lambda_dx";
          version = "0.1.0";
          src = cleanSource;

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

          postInstall = ''
            mkdir -p "$out/share/lambda_dx"
            cp -r ${cleanSource}/assets "$out/share/lambda_dx/assets"
            cp -r ${cleanSource}/songs "$out/share/lambda_dx/songs"
          '';

          postFixup = ''
            wrapProgram "$out/bin/lambda_dx_player" \
              --prefix LD_LIBRARY_PATH : "${libraryPath}" \
              --set MAI2_ASSET_DIR "$out/share/lambda_dx/assets" \
              --set MAI2_BUNDLED_SONGS_DIR "$out/share/lambda_dx/songs" \
              --set-default MAI2_FONT_PATH "${cjkFont}" \
              --set-default MAI2_FONT_INDEX "0" \
              --run 'export MAI2_DATA_DIR="''${MAI2_DATA_DIR:-''${XDG_DATA_HOME:-''$HOME/.local/share}/lambda_dx}"'
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
            rustc
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
