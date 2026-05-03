{
  description = "Flake for phphantom-lsp (local development)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
        stubsLock = pkgs.lib.importTOML ./stubs.lock;
        repoParts = pkgs.lib.splitString "/" stubsLock.repo;

        owner = builtins.elemAt repoParts 0;
        repo = builtins.elemAt repoParts 1;

        stubsTarball = pkgs.fetchurl {
          url = "https://github.com/${owner}/${repo}/archive/${stubsLock.commit}.tar.gz";

          # convert hex → SRI
          hash = builtins.convertHash {
            hash = stubsLock.sha256;
            hashAlgo = "sha256";
            toHashFormat = "sri";
          };
        };

        stubsSrc = pkgs.runCommand "phpstorm-stubs" { } ''
          mkdir -p $out
          tar -xzf ${stubsTarball} --strip-components=1 -C $out
        '';
      in
      {
        packages.default = self.packages.${system}.phpantom-lsp;
        packages.phpantom-lsp = pkgs.rustPlatform.buildRustPackage {
          pname = manifest.name;
          cargoLock.lockFile = ./Cargo.lock;
          version = manifest.version;

          # Use current directory as the source
          src = pkgs.lib.cleanSource ./.;

          postPatch = ''
            mkdir -p stubs/jetbrains
            cp -a ${stubsSrc} stubs/jetbrains/phpstorm-stubs
            chmod u+wx stubs/jetbrains/phpstorm-stubs
            echo "${stubsLock.commit}" > stubs/jetbrains/phpstorm-stubs/.commit
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
      }
    );
}
