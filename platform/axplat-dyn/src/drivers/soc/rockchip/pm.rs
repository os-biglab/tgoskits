use core::any::Any;

use rdrive::{PlatformDevice, module_driver, probe::OnProbeError, register::FdtInfo};
use rockchip_pm::{RkBoard, RockchipPM};

use crate::drivers::iomap;

module_driver!(
    name: "Rockchip Pm",
    level: ProbeLevel::PostKernel,
    priority: ProbePriority::CLK,
    probe_kinds: &[
        ProbeKind::Fdt {
            compatibles: &["rockchip,rk3588-pmu"],
            on_probe: probe
        }
    ],
);

fn probe(info: FdtInfo<'_>, plat_dev: PlatformDevice) -> Result<(), OnProbeError> {
    let base_reg = info
        .node
        .regs()
        .into_iter()
        .next()
        .ok_or(OnProbeError::other(alloc::format!(
            "[{}] has no reg",
            info.node.name()
        )))?;

    let mmio_size = base_reg.size.unwrap_or(0x1000);
    let board = RkBoard::Rk3588;
    let mmio_base = iomap((base_reg.address as usize).into(), mmio_size as usize)?;
    let pm = RockchipPmDriver(RockchipPM::new(mmio_base, board));

    plat_dev.register(pm);
    info!("Rockchip power manager registered successfully");
    Ok(())
}

struct RockchipPmDriver(RockchipPM);

impl rdrive::DriverGeneric for RockchipPmDriver {
    fn name(&self) -> &str {
        "rockchip-pm"
    }

    fn raw_any(&self) -> Option<&dyn Any> {
        Some(&self.0)
    }

    fn raw_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(&mut self.0)
    }
}
