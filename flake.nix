{
  description = "cloudflare-dyndns-rs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        cloudflare-dyndns-rs = pkgs.rustPlatform.buildRustPackage {
          pname = "cloudflare-dyndns-rs";
          version = "0.4.0";

          src = pkgs.lib.cleanSource ./.;

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            clang
            pkg-config
          ];

          buildInputs = with pkgs; [
            openssl
          ];

          meta = with pkgs.lib; {
            description = "Dynamic DNS updater that keeps Cloudflare DNS records in sync with your public IP";
            homepage = "https://github.com/Mange/cloudflare-dyndns-rs";
            license = licenses.mit;
            mainProgram = "cloudflare-dyndns-rs";
          };
        };
      in {
        packages.default = cloudflare-dyndns-rs;

        apps.default = {
          type = "app";
          program = "${cloudflare-dyndns-rs}/bin/cloudflare-dyndns-rs";
          meta = cloudflare-dyndns-rs.meta;
        };

        devShells.default = pkgs.mkShell {
          name = "cloudflare-dyndns-rs";

          buildInputs = with pkgs; [
            clang
            openssl
            pkg-config
          ];
        };
      });
}
