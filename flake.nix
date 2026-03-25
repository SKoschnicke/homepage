{
  description = "Sven's Homepage development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, fenix }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      crossPkgs = pkgs.pkgsCross.aarch64-multiplatform-musl;

      # Rust toolchain with aarch64-musl target (static binary, no glibc dependency)
      rustToolchain = fenix.packages.${system}.combine [
        fenix.packages.${system}.stable.cargo
        fenix.packages.${system}.stable.rustc
        fenix.packages.${system}.targets.aarch64-unknown-linux-musl.stable.rust-std
      ];
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = [
          rustToolchain
          pkgs.hugo
          pkgs.gcc
          pkgs.wrk
          crossPkgs.stdenv.cc
        ];

        CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER =
          "${crossPkgs.stdenv.cc}/bin/aarch64-unknown-linux-musl-gcc";
        CC_aarch64_unknown_linux_musl =
          "${crossPkgs.stdenv.cc}/bin/aarch64-unknown-linux-musl-gcc";

        shellHook = ''
          echo "Homepage dev environment loaded"
          echo "  hugo $(hugo version | grep -oP 'v\d+\.\d+\.\d+')"
          echo "  cargo $(cargo --version | cut -d' ' -f2)"
          echo "  cross-compile: cargo build --release --target aarch64-unknown-linux-musl"
        '';
      };
    };
}
