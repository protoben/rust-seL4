let
  defaultNixpkgsSource =
    let
      rev = "69c254b384fd1d2b5a032ef8177482639289b541";
      ref = "refs/tags/keep/${builtins.substring 0 32 rev}";
    in
      builtins.fetchGit {
        url = "https://gitlab.com/coliasgroup/nixpkgs.git";
        inherit rev ref;
      };

  defaultNixpkgsFn = import defaultNixpkgsSource;
  defaultNixpkgsLib = import (defaultNixpkgsSource + "/lib");
in

{ lib ? defaultNixpkgsLib, nixpkgsFn ? defaultNixpkgsFn }:

let
  makeOverridableWith = f: g: x: (g x) // {
    override = x': makeOverridableWith f g (f x' x);
  };

  mkLeaf = value: {
    __isLeaf = null;
    inherit value;
  };

  mapLeaves = f: lib.mapAttrs (k: v:
    if lib.isAttrs v
    then (
      if v ? __isLeaf
      then f v.value
      else mapLeaves f v
    )
    else v
  );

  crossSystems = {
    build = mkLeaf null;
    host =
      let
        # Avoid cache misses in cases where buildPlatform == hostPlatform
        guard = config: if config == this.pkgs.build.hostPlatform.config then null else { inherit config; };
      in {
        aarch64 = {
          none = mkLeaf (guard "aarch64-none-elf");
          linux = mkLeaf (guard "aarch64-unknown-linux-gnu");
          linuxMusl = mkLeaf (guard "aarch64-unknown-linux-musl");
        };
        aarch32 = {
          none = mkLeaf (guard "arm-none-eabi");
          linux = mkLeaf (guard "armv7l-unknown-linux-gnueabihf");
        };
        riscv64 = {
          none = mkLeaf (guard "riscv64-none-elf");
          noneWithLibc = mkLeaf (guard "riscv64-none-elf" // {
            this.noneWithLibc = true;
          });
          linux = mkLeaf (guard "riscv64-unknown-linux-gnu");
        };
        riscv32 = {
          none = mkLeaf (guard "riscv32-none-elf");
          noneWithLibc = mkLeaf (guard "riscv32-none-elf" // {
            this.noneWithLibc = true;
          });
          linux = mkLeaf (guard "riscv32-unknown-linux-gnu");
        };
        x86_64 = {
          none = mkLeaf (guard "x86_64-elf");
          linux = mkLeaf (guard "x86_64-unknown-linux-gnu");
        };
        ia32 = {
          none = mkLeaf (guard "i686-elf");
          linux = mkLeaf (guard "i686-unknown-linux-gnu");
        };
      };
  };

  baseArgs = selfThis: {
    nixpkgsArgsFor = crossSystem: {
      inherit crossSystem;
      overlays = [
        (self: super: {
          thisTopLevel = selfThis;
        })
        (import ./overlay)
      ];
    };
  };

  mkThis = args: lib.fix (self:
    let
      concreteArgs = args self;
      pkgs = mapLeaves (crossSystem:
        nixpkgsFn (concreteArgs.nixpkgsArgsFor crossSystem)
      ) crossSystems;
    in {
      inherit lib pkgs;
    } // import ./top-level self);

  this = makeOverridableWith lib.id mkThis baseArgs;

in
  this
