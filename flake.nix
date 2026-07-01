{
  description = "A TUI for attributing network usage to processes via eBPF";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (
      system:
      let
        pkgs = import nixpkgs { inherit system; };

        buildDeps = with pkgs; [
          bpftools
          clang
          elfutils
          libbpf
          pkg-config
          rustup
          zlib
        ];

        devDeps = with pkgs; [
          clang-tools
          gnumake
          rust-analyzer
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          packages = buildDeps ++ devDeps;

          CPATH = pkgs.lib.makeSearchPathOutput "dev" "include" [
            pkgs.libbpf
          ];
        };
      }
    );
}
