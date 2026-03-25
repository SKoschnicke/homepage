{
  description = "Sven's Homepage development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          hugo
          cargo
          rustc
          gcc  # needed for linking
          wrk  # load testing
        ];

        shellHook = ''
          echo "Homepage dev environment loaded"
          echo "  hugo $(hugo version | grep -oP 'v\d+\.\d+\.\d+')"
          echo "  cargo $(cargo --version | cut -d' ' -f2)"
        '';
      };
    };
}
