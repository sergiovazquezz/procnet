{
  description = "A TUI for attributing network usage to processes via eBPF.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { nixpkgs, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
      };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [
          clang
          clang-tools
          llvm
          libbpf
          bpftools
          pkg-config
          elfutils
          rustup
          rust-analyzer
          zlib
          gnumake
          linuxHeaders
        ];

        CPATH = pkgs.lib.makeSearchPathOutput "dev" "include" [
          pkgs.libbpf
          pkgs.linuxHeaders
        ];
      };
    };
}
