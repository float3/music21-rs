{
  description = "Nix flake for music21-rs development";

  inputs = {
        nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        lib = pkgs.lib;
        python = pkgs.python3;
        pythonPackages = python.pkgs;
        linuxOnlyLibs = with pkgs; lib.optionals stdenv.isLinux [alsa-lib];
        libclangInclude = "-I${pkgs.llvmPackages_latest.libclang.lib}/lib/clang/${pkgs.llvmPackages_latest.libclang.version}/include";
      in {
        devShells.default = pkgs.mkShell {
          packages =
            (with pkgs; [
              cargo
              rustc
              clippy
              rustfmt
              alejandra
              git
              pkg-config
              clang
              llvmPackages_latest.libclang
              openssl
              python
              pythonPackages.virtualenv
              pythonPackages.requests
            ])
            ++ linuxOnlyLibs;

          # https://github.com/rust-lang/rust-bindgen#environment-variables
          LIBCLANG_PATH = "${pkgs.llvmPackages_latest.libclang.lib}/lib";

          BINDGEN_EXTRA_CLANG_ARGS = lib.concatStringsSep " " (
            [libclangInclude]
            ++ lib.optionals pkgs.stdenv.isLinux ["-I${pkgs.glibc.dev}/include"]
          );

          LD_LIBRARY_PATH = lib.makeLibraryPath (
            [pkgs.openssl pkgs.llvmPackages_latest.libclang]
            ++ linuxOnlyLibs
          );

          PYO3_PYTHON = python.interpreter;

          shellHook = ''
            echo "Entered music21-rs dev shell for ${system}"
          '';
        };

        checks.alejandra = pkgs.runCommand "alejandra-check" {nativeBuildInputs = [pkgs.alejandra];} ''
          alejandra --check ${./flake.nix}
          touch "$out"
        '';

        formatter = pkgs.alejandra;
      }
    );
}
