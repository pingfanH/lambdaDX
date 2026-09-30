{
  description = "LambdaDX";

  nixConfig = {
    extra-substituters = ["https://lnmai-core.cachix.org"];
    extra-trusted-public-keys = [
      "lnmai-core.cachix.org-1:rYcjvGbYnD1X9NWUExTn2dly2tFFzuamDEj02rJG7F8="
    ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    nixgl = {
      url = "github:nix-community/nixGL?rev=b6105297e6f0cd041670c3e8628394d4ee247ed5";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
    lnmai-core-ffi = {
      url = "github:pingfanH/lnmai-core-ffi?ref=master";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
      inputs.lnmai-core.follows = "lnmai-core";
    };
    lnmai-core = {
      url = "github:Neuron-Group/lnmai-core?rev=4efad7ea302f787bff7c40f3b255443808d7e4cb";
      inputs.nixpkgs.url = "github:NixOS/nixpkgs/567a49d1913ce81ac6e9582e3553dd90a955875f";
    };
    maisimai = {
      url = "github:pingfanH/maisimai-rs?ref=master";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    nixgl,
    lnmai-core-ffi,
    lnmai-core,
    maisimai,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {inherit system;};
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
      stagedSource =
        pkgs.runCommand "lambdadx-source" {
          nativeBuildInputs = [pkgs.rsync];
        } ''
          mkdir -p \
            "$out/maisimai" \
            "$out/lnmai-core-ffi" \
            "$out/lnmai-core-ffi/lnmai-core"

          rsync -a --chmod=Du+w,Dgo+rx,Fu+w,Fgo+r \
            --exclude .git/ \
            --exclude target/ \
            --exclude result \
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
            ${lnmai-core-ffi}/. "$out/lnmai-core-ffi/"
          rsync -a --chmod=Du+w,Dgo+rx,Fu+w,Fgo+r \
            --exclude .git/ \
            --exclude target/ \
            --exclude result \
            --exclude .lake/ \
            ${lnmai-core}/. "$out/lnmai-core-ffi/lnmai-core/"

          chmod -R u+w "$out"
        '';
      lambdaDxPlayer = pkgs.rustPlatform.buildRustPackage {
        pname = "lambda_dx";
        version = "0.1.0";
        src = stagedSource;

        cargoLock.lockFile = ./Cargo.lock;
        cargoBuildFlags = ["--bin" "lambda_dx_player_ui"];
        cargoInstallFlags = ["--bin" "lambda_dx_player_ui"];

        nativeBuildInputs = with pkgs; [
          makeWrapper
          pkg-config
        ];
        buildInputs = libs ++ devLibs;

        LNMAI_CORE_ARTIFACTS = "${lnmaiCoreArtifacts}";
        LNMAI_CORE_LEAN_PROJECT = "${stagedSource}/lnmai-core-ffi/lnmai-core";
        LIBRARY_PATH = libraryPath;
        LD_LIBRARY_PATH = libraryPath;
        PKG_CONFIG_PATH = pkgConfigPath;

        doCheck = false;

        postFixup = ''
          mkdir -p "$out/share/lambda_dx"
          cp -r "$src/assets" "$out/share/lambda_dx/assets"
          wrapProgram "$out/bin/lambda_dx_player_ui" \
            --suffix LD_LIBRARY_PATH : "${libraryPath}" \
            --run 'if [ -n "''${WAYLAND_DISPLAY:-}" ] && [ -z "''${EGL_PLATFORM:-}" ]; then export EGL_PLATFORM=wayland; fi' \
            --set MAI2_ASSET_DIR "$out/share/lambda_dx/assets" \
            --set MAI2_FONT_PATH "${cjkFontPath}"
        '';
      };
      playerNixgl = pkgs.writeShellScriptBin "lambda-dx-player-nixgl" ''
        exec ${nixgl.packages.${system}.nixGLIntel}/bin/nixGLIntel \
          ${lambdaDxPlayer}/bin/lambda_dx_player_ui "$@"
      '';
      commonEnv = ''
        export RUST_BACKTRACE=1
        export LNMAI_CORE_ARTIFACTS="${lnmaiCoreArtifacts}"
        export LNMAI_CORE_LEAN_PROJECT="${stagedSource}/lnmai-core-ffi/lnmai-core"
        export MAI2_FONT_PATH="${cjkFontPath}"
        export LD_LIBRARY_PATH="''${LD_LIBRARY_PATH:+$LD_LIBRARY_PATH:}${libraryPath}"
        export LIBRARY_PATH="${libraryPath}''${LIBRARY_PATH:+:$LIBRARY_PATH}"
        export PKG_CONFIG_PATH="${pkgConfigPath}''${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
      '';
    in {
      packages.default = lambdaDxPlayer;
      packages.player = lambdaDxPlayer;
      packages.player-nixgl = playerNixgl;

      apps.default = {
        type = "app";
        program = "${lambdaDxPlayer}/bin/lambda_dx_player_ui";
      };

      apps.player = {
        type = "app";
        program = "${lambdaDxPlayer}/bin/lambda_dx_player_ui";
      };

      apps.player-nixgl = {
        type = "app";
        program = "${playerNixgl}/bin/lambda-dx-player-nixgl";
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
        LNMAI_CORE_LEAN_PROJECT = "${stagedSource}/lnmai-core-ffi/lnmai-core";
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
