{
  description = "Flake for phphantom-lsp (local development)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
      in
      {
        packages.default = self.packages.${system}.phpantom-lsp;
        packages.phpantom-lsp = pkgs.rustPlatform.buildRustPackage rec {
          pname = manifest.name;
          cargoLock.lockFile = ./Cargo.lock;
          version = manifest.version;

          # Use current directory as the source
          src = pkgs.lib.cleanSource ./.;

          stubsSrc = pkgs.fetchFromGitHub {
            owner = "JetBrains";
            repo = "phpstorm-stubs";
            rev = "3327932472f512d2eb9e122b19702b335083fd9d";
            hash = "sha256-WN5DAvaw4FfHBl2AqSo1OcEthUm3lOpikdB78qy3cyY=";
          };

          postPatch = ''
            mkdir -p stubs/jetbrains
            cp -a ${stubsSrc} stubs/jetbrains/phpstorm-stubs
            chmod u+wx stubs/jetbrains/phpstorm-stubs
            echo "${stubsSrc.rev}" > stubs/jetbrains/phpstorm-stubs/.commit
          '';

          checkFlags = [
            "--test"
            "completion_inheritance"
          ];

          postInstall = ''
            mv $out/bin/phpantom_lsp $out/bin/phpantom-lsp
            ln -s $out/bin/phpantom-lsp $out/bin/phpantom_lsp
          '';
        };
      });
}
