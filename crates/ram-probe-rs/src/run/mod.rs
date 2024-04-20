mod arm;
#[cfg(feature = "defmt")]
mod defmt;
mod rtt;

use crate::elf::{Segments, VectorTable};
#[cfg(feature = "defmt")]
pub use defmt::{DefmtOpts, DefmtRunner};
use eyre::{bail, eyre, Result};
use probe_rs::rtt::{Rtt, ScanRegion};
use probe_rs::{MemoryInterface as _, Session};
use std::time::Duration;

pub fn init_cpu(
    session: &mut Session,
    segments: &Segments,
    vector_table: &VectorTable,
    timeout: Duration,
) -> Result<()> {
    // Validate the main core supports RTT.
    if session.core(0)?.available_breakpoint_units()? == 0 {
        bail!("RTT not supported on device without HW breakpoints");
    }

    // Reset ALL cores other than the main one.
    for (i, _) in session.list_cores() {
        if i != 0 {
            log::debug!("resetting core `{}`", i);
            session.core(i)?.reset()?;
        }
    }

    // Reset and halt the main core.
    let mut core = session.core(0)?;
    log::debug!("resetting and halting core 0");
    core.reset_and_halt(timeout)?;

    // Write RAM code.
    log::info!("writing ram");
    for (address, segment) in segments.iter() {
        core.write_8(*address, segment)?;
    }
    log::info!("wrote ram");

    // Init CPU to RAM code.
    log::debug!("initializing CPU");
    let pc = core.program_counter().id();
    let sp = core.stack_pointer().id();

    // Reset CPU to run RAM code.
    core.write_core_reg(pc, vector_table.reset)?;
    core.write_core_reg(sp, vector_table.initial_sp)?;
    // Write VTOR location for RAM vector table.
    core.write_word_32(arm::VTOR, vector_table.address)?;

    // Patch the hard fault to trigger a break point.
    core.write_8(
        arm::thumb_v7_align!(vector_table.hard_fault) as _,
        arm::BKPT_ASM,
    )?;

    log::debug!("restarting CPU");
    core.run()?;

    Ok(())
}

pub fn setup_rtt(session: &mut Session, rtt_addr: u32, retries: usize) -> Result<Rtt> {
    let mut rtt_res: Result<Rtt, probe_rs::rtt::Error> =
        Err(probe_rs::rtt::Error::ControlBlockNotFound);

    let memory_map = session.target().memory_map.clone();
    let mut core = session.core(0).unwrap();

    for try_index in 0..=retries {
        rtt_res = Rtt::attach_region(&mut core, &memory_map, &ScanRegion::Exact(rtt_addr));
        match rtt_res {
            Ok(_) => {
                log::debug!("successfully attached RTT");
                break;
            }
            Err(probe_rs::rtt::Error::ControlBlockNotFound) => {
                if try_index < retries {
                    log::trace!(
                        "could not attach because the target's RTT control block isn't initialized (yet). retrying"
                    );
                } else {
                    log::error!("max number of RTT attach retries exceeded.");
                    return Err(eyre!(probe_rs::rtt::Error::ControlBlockNotFound));
                }
            }
            Err(e) => {
                return Err(eyre!(e));
            }
        }
    }

    // this block is only executed when rtt was successfully attached before
    let mut rtt = rtt_res.expect("unreachable");
    for ch in rtt.up_channels().iter() {
        log::debug!(
            "up channel {}: {:?}, buffer size {} bytes",
            ch.number(),
            ch.name(),
            ch.buffer_size()
        );
    }
    for ch in rtt.down_channels().iter() {
        log::debug!(
            "down channel {}: {:?}, buffer size {} bytes",
            ch.number(),
            ch.name(),
            ch.buffer_size()
        );
    }

    Ok(rtt)
}
