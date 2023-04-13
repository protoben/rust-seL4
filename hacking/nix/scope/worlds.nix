{ lib, pkgsBuildBuild
, configHelpers
, mkWorld
}:

with configHelpers;

let
  loaderConfig = {};

  kernelConfigCommon = {
    KernelVerificationBuild = off;
    KernelRootCNodeSizeBits = mkString "14"; # For backtrace test with embedded debug info
  };

in rec {
  aarch64 =
    let
    in rec {
      default = qemu-arm-virt.el2;
      qemu-arm-virt =
        let
          mk = { smp ? false, mcs ? off, hypervisor ? false, cpu ? "cortex-a57", forCP ? false }:
            let
              numCores = if smp then "2" else "1";
            in
              mkWorld {
                inherit loaderConfig;
                inherit forCP;
                kernelConfig = kernelConfigCommon // {
                  ARM_CPU = mkString cpu;
                  KernelArch = mkString "arm";
                  KernelSel4Arch = mkString "aarch64";
                  KernelPlatform = mkString "qemu-arm-virt";
                  KernelMaxNumNodes = mkString numCores;
                  KernelIsMCS = mcs;
                } // lib.optionalAttrs hypervisor {
                  KernelArmHypervisorSupport = on;
                };
                mkQemuCmd = loader: [
                  # NOTE
                  # virtualization=on even when hypervisor to test loader dropping exception level
                  "${pkgsBuildBuild.qemu}/bin/qemu-system-aarch64"
                    "-machine" "virt,virtualization=on"
                    "-cpu" cpu "-smp" numCores "-m" "1024"
                    "-nographic"
                    "-serial" "mon:stdio"
                    "-kernel" loader
                ];
              };
        in rec {
          default = el2;
          el1 = mk { smp = true; };
          el2 = mk { smp = true; hypervisor = true; };
          el2MCS = mk { smp = true; hypervisor = true; mcs = on; };
          sel4cp = mk { mcs = on; forCP = true; cpu = "cortex-a53"; };
        };

      bcm2711 = mkWorld {
        inherit loaderConfig;
        kernelConfig = kernelConfigCommon // {
          ARM_CPU = mkString "cortex-a57";
          KernelArch = mkString "arm";
          KernelSel4Arch = mkString "aarch64";
          KernelPlatform = mkString "bcm2711";
          KernelArmHypervisorSupport = on;
          # KernelMaxNumNodes = mkString "2"; # TODO
        };
      };
    };

  riscv64 =
    let
    in rec {
      default = spike;

      spike = mkWorld {
        inherit loaderConfig;
        kernelConfig = kernelConfigCommon // {
          KernelArch = mkString "riscv";
          KernelSel4Arch = mkString "riscv64";
          KernelPlatform = mkString "spike";
        };
      };
    };

  riscv32 =
    let
    in rec {
      default = spike;

      spike = mkWorld {
        inherit loaderConfig;
        kernelConfig = kernelConfigCommon // {
          KernelArch = mkString "riscv";
          KernelSel4Arch = mkString "riscv32";
          KernelPlatform = mkString "spike";
        };
      };
    };

  x86 = is64bit:
    let
    in rec {
      default = pc99;

      pc99 = lib.fix (self: mkWorld {
        inherit loaderConfig;
        kernelConfig = kernelConfigCommon // {
          KernelArch = mkString "x86";
          KernelSel4Arch = mkString (if is64bit then "x86_64" else "ia32");
          KernelPlatform = mkString "pc99";

          # for the sake of simulation (see seL4_tools/cmake-tool/helpers/application_settings.cmake)
          KernelFSGSBase = mkString "msr";
          # KernelFSGSBase = mkString "inst";
          KernelSupportPCID = off;
          KernelIOMMU = off;
          KernelFPU = mkString "FXSAVE";
        };
        qemuCmdRequiresLoader = false;
        mkQemuCmd =
          let
            enable = opt: "+${opt}";
            disable = opt: "-${opt}";
            opts = lib.concatStringsSep "," [
              (disable "vme")
              (enable "pdpe1gb")
              (disable "xsave")
              (disable "xsaveopt")
              (disable "xsavec")
              (disable "fsgsbase")
              # (enable "fsgsbase")
              (disable "invpcid")
              (enable "syscall")
              (enable "lm")
            ];
          in task: [
            "${pkgsBuildBuild.qemu}/bin/qemu-system-${if is64bit then "x86_64" else "i386"}"
              "-cpu" "Nehalem,${opts},enforce"
              "-m" "size=512M"
              "-nographic"
              "-serial" "mon:stdio"
              "-kernel" self.kernel32Bit
              "-initrd" task
          ];
      });
    };

  x86_64 = x86 true;
  ia32 = x86 false;

  aarch32 =
    let
    in rec {
      default = qemu-arm-virt.default;
      qemu-arm-virt =
        let
          mk = { smp ? false, mcs ? off, cpu ? "cortex-a15" }:
            let
              numCores = if smp then "2" else "1";
            in
              mkWorld {
                inherit loaderConfig;
                inherit forCP;
                kernelConfig = kernelConfigCommon // {
                  ARM_CPU = mkString cpu;
                  KernelArch = mkString "arm";
                  KernelSel4Arch = mkString "aarch32";
                  KernelPlatform = mkString "qemu-arm-virt";
                  KernelMaxNumNodes = mkString numCores;
                  KernelIsMCS = mcs;
                };
                mkQemuCmd = loader: [
                  # NOTE
                  # virtualization=on even when hypervisor to test loader dropping exception level
                  "${pkgsBuildBuild.qemu}/bin/qemu-system-aarch32"
                    "-machine" "virt"
                    "-cpu" cpu "-smp" numCores "-m" "1024"
                    "-nographic"
                    "-serial" "mon:stdio"
                    "-kernel" loader
                ];
              };
        in rec {
          default = mk {};
        };
    };

}
