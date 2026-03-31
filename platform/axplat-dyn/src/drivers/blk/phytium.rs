use alloc::{boxed::Box, format, sync::Arc, vec::Vec};
use core::{cmp, ptr::NonNull, time::Duration};

use axklib::time::busy_wait;
use log::{debug, info, trace};
pub use phytium_mci::set_impl;
use phytium_mci::{IoPad, Kernel, PAD_ADDRESS, mci_host::err::MCIHostError, sd::SdCard};
use rd_block::{BlkError, IQueue, Interface, Request, RequestId};
use rdrive::{
    DriverGeneric, PlatformDevice, module_driver, probe::OnProbeError, register::FdtInfo,
};
use spin::Mutex;

use super::PlatformDeviceBlock;
use crate::drivers::iomap;

const OFFSET: usize = 0x400_0000;
const BLOCK_SIZE: usize = 512;

pub struct KernelImpl;

impl Kernel for KernelImpl {
    fn sleep(us: Duration) {
        busy_wait(us);
    }
}

set_impl!(KernelImpl);

module_driver!(
    name: "Phytium SdCard",
    level: ProbeLevel::PostKernel,
    priority: ProbePriority::DEFAULT,
    probe_kinds: &[
        ProbeKind::Fdt {
            compatibles: &["phytium,mci"],
            on_probe: probe_sdcard
        }
    ],
);

fn probe_sdcard(info: FdtInfo<'_>, plat_dev: PlatformDevice) -> Result<(), OnProbeError> {
    info!("Probing Phytium SDCard...");
    let mci_reg = info
        .node
        .regs()
        .into_iter()
        .next()
        .ok_or(OnProbeError::other(alloc::format!(
            "[{}] has no reg",
            info.node.name()
        )))?;

    info!(
        "MCI reg: addr={:#x}, size={:#x}",
        mci_reg.address as usize,
        mci_reg.size.unwrap_or(0)
    );

    let mci_reg_base = iomap(
        (mci_reg.address as usize).into(),
        mci_reg.size.unwrap_or(0x10000) as usize,
    )
    .expect("Failed to iomap mci reg");

    let iopad_reg_base =
        iomap((PAD_ADDRESS as usize).into(), 0x2000).expect("Failed to iomap iopad reg");

    info!(
        "MCI reg base mapped at {:#x}",
        mci_reg_base.as_ptr() as usize
    );

    let mci_reg = NonNull::new(mci_reg_base.as_ptr()).expect("Failed to create NonNull pointer");
    let iopad_reg =
        NonNull::new(iopad_reg_base.as_ptr()).expect("Failed to create NonNull pointer for iopad");
    let iopad = IoPad::new(iopad_reg);

    info!("MCI reg mapped at {:p}", mci_reg);

    let sdcard = SdCardDriver::new(mci_reg, iopad);
    plat_dev.register_block(sdcard);

    debug!("phytium block device registered successfully");

    Ok(())
}

pub struct SdCardDriver {
    sd_card: Arc<SharedSdCard>,
}

impl SdCardDriver {
    pub fn new(sd_addr: NonNull<u8>, iopad: IoPad) -> Self {
        let sd_card = Arc::new(SharedSdCard(Mutex::new(Box::new(SdCard::new(
            sd_addr, iopad,
        )))));
        Self { sd_card }
    }
}

struct SharedSdCard(Mutex<Box<SdCard>>);

unsafe impl Send for SharedSdCard {}
unsafe impl Sync for SharedSdCard {}

impl DriverGeneric for SdCardDriver {
    fn name(&self) -> &str {
        "phytium-sdcard"
    }
}

impl Interface for SdCardDriver {
    fn create_queue(&mut self) -> Option<Box<dyn IQueue>> {
        Some(Box::new(SdCardQueue {
            sd_card: Arc::clone(&self.sd_card),
        }))
    }

    fn enable_irq(&mut self) {
        todo!()
    }

    fn disable_irq(&mut self) {
        todo!()
    }

    fn is_irq_enabled(&self) -> bool {
        false
    }

    fn handle_irq(&mut self) -> rd_block::Event {
        rd_block::Event::none()
    }
}

pub struct SdCardQueue {
    sd_card: Arc<SharedSdCard>,
}

impl IQueue for SdCardQueue {
    fn num_blocks(&self) -> usize {
        self.sd_card.0.lock().block_count() as usize
    }

    fn block_size(&self) -> usize {
        self.sd_card.0.lock().block_size() as usize
    }

    fn id(&self) -> usize {
        0
    }

    fn buff_config(&self) -> rd_block::BuffConfig {
        rd_block::BuffConfig {
            dma_mask: u64::MAX,
            align: 0x1000,
            size: self.block_size(),
        }
    }

    fn submit_request(&mut self, request: Request<'_>) -> Result<RequestId, BlkError> {
        let actual_block_id = request.block_id + OFFSET / BLOCK_SIZE;

        match request.kind {
            rd_block::RequestKind::Read(mut buffer) => {
                trace!("read block {actual_block_id}");
                Self::validate_buffer(&buffer)?;

                let (_, aligned_buf, _) = unsafe { buffer.align_to_mut::<u32>() };
                let mut temp_buf: Vec<u32> = Vec::with_capacity(aligned_buf.len());

                self.sd_card
                    .0
                    .lock()
                    .read_blocks(&mut temp_buf, actual_block_id as u32, 1)
                    .map_err(map_mci_error_to_blk_error)?;

                let copy_len = cmp::min(temp_buf.len(), aligned_buf.len());
                aligned_buf[..copy_len].copy_from_slice(&temp_buf[..copy_len]);

                Ok(RequestId::new(0))
            }
            rd_block::RequestKind::Write(buffer) => {
                trace!("write block {actual_block_id}");
                Self::validate_buffer(buffer)?;

                let (_, aligned_buf, _) = unsafe { buffer.align_to::<u32>() };
                let mut write_buf: Vec<u32> = aligned_buf.to_vec();

                self.sd_card
                    .0
                    .lock()
                    .write_blocks(&mut write_buf, actual_block_id as u32, 1)
                    .map_err(map_mci_error_to_blk_error)?;

                Ok(RequestId::new(0))
            }
        }
    }

    fn poll_request(&mut self, _request: RequestId) -> Result<(), BlkError> {
        Ok(())
    }
}

impl SdCardQueue {
    fn validate_buffer(buffer: &[u8]) -> Result<(), BlkError> {
        if buffer.len() < BLOCK_SIZE {
            return Err(BlkError::Other(
                format!(
                    "Invalid buffer size: expected at least {BLOCK_SIZE}, got {}",
                    buffer.len()
                )
                .into(),
            ));
        }

        let (prefix, _, suffix) = unsafe { buffer.align_to::<u32>() };
        if !prefix.is_empty() || !suffix.is_empty() {
            return Err(BlkError::Other("Invalid buffer alignment".into()));
        }

        Ok(())
    }
}

fn map_mci_error_to_blk_error(err: MCIHostError) -> BlkError {
    match err {
        MCIHostError::Timeout => BlkError::Retry,
        MCIHostError::InvalidArgument => BlkError::Other("Invalid argument".into()),
        MCIHostError::Busy | MCIHostError::NoData | MCIHostError::NoTransferInProgress => {
            BlkError::Retry
        }
        other => BlkError::Other(alloc::format!("MCI error: {other:?}").into()),
    }
}
