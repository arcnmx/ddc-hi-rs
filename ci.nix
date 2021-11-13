{ config, channels, pkgs, lib, ... }: with pkgs; with lib; let
  mingwW64-target = channels.rust.lib.targetForConfig.${lib.systems.examples.mingwW64.config};
  rustChannel = channels.rust.stable.override {
    channelOverlays = [
      (cself: csuper: {
        sysroot-std = csuper.sysroot-std ++ [ cself.manifest.targets.${mingwW64-target}.rust-std ];
      })
    ];
  };
  importShell = channels.cipkgs.writeText "shell.nix" ''
    import ${builtins.unsafeDiscardStringContext config.shell.drvPath}
  '';
  build = channels.cipkgs.ci.command {
    name = "cargo-build";
    command = ''
      nix-shell ${importShell} --run "cargo build"
    '';
    impure = true;
  };
  build-windows = channels.cipkgs.ci.command {
    name = "cargo-build-windows";
    command = ''
      nix-shell ${importShell} --run "cargo build --target ${mingwW64-target}"
    '';
    impure = true;
  };
in {
  config = {
    name = "ddc-hi";
    ci.gh-actions.enable = true;
    cache.cachix.arc.enable = true;
    channels = {
      nixpkgs = "21.11";
      rust = "master";
    };
    environment = {
      test = {
        inherit (config.rustChannel.buildChannel) cargo;
      };
    };
    tasks = {
      build.inputs = singleton build;
    };
    jobs = {
      nixos = {
        tasks.windows.inputs = singleton build-windows;
      };
      macos.system = "x86_64-darwin";
    };
  };

  options = {
    rustChannel = mkOption {
      type = types.unspecified;
      default = rustChannel;
    };
    shell = mkOption {
      type = types.unspecified;
      default = with pkgs; config.rustChannel.mkShell {
        buildInputs =
          optional pkgs.hostPlatform.isLinux udev
          ++ optional pkgs.hostPlatform.isDarwin libiconv;
        nativeBuildInputs = optional pkgs.hostPlatform.isLinux pkg-config;
      };
    };
  };
}
