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
  test = channels.cipkgs.ci.command {
    name = "cargo-test";
    command = ''
      nix-shell ${importShell} --run "cargo test"
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
      build.inputs = [ build test ];
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
          optional hostPlatform.isLinux udev
          ++ optionals hostPlatform.isDarwin [ libiconv darwin.apple_sdk.frameworks.CoreGraphics ];
        nativeBuildInputs = optional hostPlatform.isLinux pkg-config;
      };
    };
  };
}
