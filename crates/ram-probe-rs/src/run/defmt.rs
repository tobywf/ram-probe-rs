use crate::defmt::{DefmtDecoder, DefmtInfo};
use crate::elf::{Segments, VectorTable};
use eyre::{eyre, Result};
use probe_rs::rtt::UpChannel;
use probe_rs::Session;
use std::time::Duration;

pub struct DefmtOpts<'a> {
    pub segments: &'a Segments<'a>,
    pub rtt_addr: u32,
    pub vector_table: &'a VectorTable,
    pub defmt: &'a DefmtInfo,
    pub timeout: Duration,
    pub retries: usize,
}

impl<'a> DefmtOpts<'a> {
    pub fn with_defaults(
        segments: &'a Segments<'a>,
        rtt_addr: u32,
        vector_table: &'a VectorTable,
        defmt: &'a DefmtInfo,
    ) -> Self {
        Self {
            segments,
            rtt_addr,
            vector_table,
            defmt,
            timeout: Duration::from_secs(1),
            retries: 10,
        }
    }
}

pub struct DefmtRunner<'opts> {
    decoder: DefmtDecoder<'opts>,
    defmt: UpChannel,
}

impl<'opts> DefmtRunner<'opts> {
    pub fn new(session: &mut Session, opts: &'opts DefmtOpts<'_>) -> Result<Self> {
        super::init_cpu(session, &opts.segments, &opts.vector_table, opts.timeout)?;

        let mut rtt = super::setup_rtt(session, opts.rtt_addr, opts.retries)?;

        let defmt = rtt
            .up_channels()
            .take(0)
            .ok_or_else(|| eyre!("RTT up channel 0 not found"))?;

        let decoder = DefmtDecoder::new(&opts.defmt, "target");
        Ok(Self { decoder, defmt })
    }

    pub fn run(&mut self, session: &mut Session) -> Result<()> {
        let mut was_halted = false;

        loop {
            self.poll(session)?;

            let mut core = session.core(0)?;
            let is_halted = core.core_halted()?;

            if is_halted && was_halted {
                return Ok(());
            }
            was_halted = is_halted;
        }
    }

    pub fn poll(&mut self, session: &mut Session) -> Result<()> {
        let mut read_buf = [0; 1024];
        let n = self
            .defmt
            .read(&mut session.core(0).unwrap(), &mut read_buf)?;

        self.decoder.decode(&read_buf[..n])
    }
}
