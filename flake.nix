{
  description = "A TUI for attributing network usage to processes via eBPF";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      inherit (nixpkgs) lib;
      systems = lib.intersectLists lib.systems.flakeExposed lib.platforms.linux;
      forAllSystems = lib.genAttrs systems;
      nixpkgsFor = forAllSystems (
        system:
        import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        }
      );

      nightlyRustPlatform =
        system:
        let
          pkgs = nixpkgsFor.${system};
        in
        pkgs.makeRustPlatform {
          rustc = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          cargo = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        };

      procnet-package =
        system:
        {
          lib,
          clang,
          pkg-config,
          elfutils,
          libbpf,
          zlib,
        }:
        (nightlyRustPlatform system).buildRustPackage {
          pname = "procnet";
          version = "0.1.0";

          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./crates
              ./Cargo.toml
              ./Cargo.lock
              ./.cargo/config.toml
            ];
          };

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [
            clang
            pkg-config
          ];

          buildInputs = [
            elfutils
            libbpf
            zlib
          ];

          LD_LIBRARY_PATH = lib.makeLibraryPath [
            elfutils
            libbpf
            zlib
          ];

          RUSTFLAGS = "-C link-arg=-Wl,-rpath,${
            lib.makeLibraryPath [
              elfutils
              zlib
            ]
          }";

          PROCNET_SKIP_VMLINUX_GEN = "1";

          doCheck = false;

          meta = {
            description = "TUI for attributing network usage to processes via eBPF";
            homepage = "https://github.com/sergiovazquezz/procnet";
            license = lib.licenses.gpl2Only;
            mainProgram = "procnet";
            platforms = lib.platforms.linux;
          };
        };

      nixosModule =
        {
          pkgs,
          config,
          lib,
          ...
        }:
        let
          cfg = config.services.procnet;
          system = pkgs.stdenv.hostPlatform.system;
          pkg = nixpkgsFor.${system}.callPackage (procnet-package system) { };
        in
        {
          options.services.procnet.enable = lib.mkEnableOption "procnet eBPF network-usage daemon and TUI client";

          config = lib.mkIf cfg.enable {
            environment.systemPackages = [ pkg ];

            security.wrappers.procnetd = {
              source = "${pkg}/bin/procnetd";
              owner = "root";
              group = "root";
              capabilities = "cap_bpf,cap_perfmon,cap_sys_resource+ep";
            };

            systemd.user.services.procnetd = {
              description = "Procnet eBPF network-usage daemon";
              documentation = [ "https://github.com/sergiovazquezz/procnet" ];
              after = [ "network.target" ];
              wantedBy = [ "default.target" ];
              serviceConfig = {
                Type = "simple";
                ExecStart = "${config.security.wrapperDir}/procnetd";
                StandardOutput = "journal";
                StandardError = "journal";
              };
            };
          };
        };
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgsFor.${system};
          pkg = pkgs.callPackage (procnet-package system) { };
        in
        {
          default = pkgs.mkShell {
            packages =
              pkg.buildInputs
              ++ pkg.nativeBuildInputs
              ++ [
                pkgs.rustup
                pkgs.clang-tools
                pkgs.gnumake
                pkgs.rust-analyzer
                pkgs.bpftools
              ];

            CPATH = lib.makeSearchPathOutput "dev" "include" [ pkgs.libbpf ];
          };
        }
      );

      nixosModules.default = nixosModule;
    };
}
