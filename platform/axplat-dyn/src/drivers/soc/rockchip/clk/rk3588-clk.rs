use rdrive::{
    DriverGeneric, PlatformDevice, module_driver, probe::OnProbeError, register::FdtInfo,
};
use rk3588_clk::Rk3588Cru;

use crate::drivers::iomap;

module_driver!(
    name: "Rockchip CRU",
    level: ProbeLevel::PostKernel,
    priority: ProbePriority::CLK,
    probe_kinds: &[
        ProbeKind::Fdt {
            compatibles: &["rockchip,rk3588-cru"],
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
    let mmio_base = iomap((base_reg.address as usize).into(), mmio_size as usize)?;

    let cru = Rk3588Cru::new(mmio_base);
    let clk = rdif_clk::Clk::new(ClkDrv::new(cru));

    plat_dev.register(clk);
    info!("clk registered successfully");
    Ok(())
}

pub struct ClkDrv {
    inner: Rk3588Cru,
}

impl ClkDrv {
    pub fn new(cru: Rk3588Cru) -> Self {
        cru.init();
        Self { inner: cru }
    }
}

unsafe impl Send for ClkDrv {}

impl DriverGeneric for ClkDrv {
    fn name(&self) -> &str {
        "rk3588-clk"
    }
}

impl rdif_clk::Interface for ClkDrv {
    fn perper_enable(&mut self) {}

    fn get_rate(&self, id: rdif_clk::ClockId) -> Result<u64, rdrive::KError> {
        let id: usize = id.into();
        let rate = self
            .inner
            .mmc_get_clk(id as _)
            .map_err(|_| rdrive::KError::InvalidArg { name: "id" })?;
        Ok(rate as _)
    }

    fn set_rate(&mut self, id: rdif_clk::ClockId, rate: u64) -> Result<(), rdrive::KError> {
        let id: usize = id.into();
        self.inner
            .mmc_set_clk(id as _, rate as _)
            .map_err(|_| rdrive::KError::InvalidArg { name: "id" })?;
        Ok(())
    }
}
