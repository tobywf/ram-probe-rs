use crate::defmt::DefmtInfo;
use crate::elf::{Segments, VectorTable};
use defmt_decoder::{DecodeError, Location, StreamDecoder, Table};
use eyre::{bail, eyre, Result};
use probe_rs::rtt::{Rtt, UpChannel};
use probe_rs::Session;
use std::collections::BTreeMap;
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
    table: &'opts Table,
    locations: &'opts BTreeMap<u64, Location>,
    stream: Box<dyn StreamDecoder + 'opts>,
    defmt: UpChannel,
    pub rtt: Rtt,
}

impl<'opts> DefmtRunner<'opts> {
    pub fn new(session: &mut Session, opts: &'opts DefmtOpts<'_>) -> Result<Self> {
        super::init_cpu(session, &opts.segments, &opts.vector_table, opts.timeout)?;

        let mut rtt = super::setup_rtt(session, opts.rtt_addr, opts.retries)?;

        let defmt = rtt
            .up_channels()
            .take(0)
            .ok_or_else(|| eyre!("RTT up channel 0 not found"))?;

        let stream = opts.defmt.table.new_stream_decoder();

        let DefmtInfo { table, locations } = &opts.defmt;

        Ok(Self {
            table,
            locations,
            stream,
            defmt,
            rtt,
        })
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
        self.stream.received(&read_buf[..n]);

        loop {
            match self.stream.decode() {
                Ok(frame) => {
                    let loc = self.locations.get(&frame.index());

                    let (mut file, mut line) = (None, None);
                    if let Some(loc) = loc {
                        file = Some(loc.file.display().to_string());
                        line = Some(loc.line as u32);
                    };

                    let mut timestamp = String::new();
                    if let Some(ts) = frame.display_timestamp() {
                        timestamp = format!("{} ", ts);
                    }

                    log::logger().log(
                        &log::Record::builder()
                            .level(match frame.level() {
                                Some(level) => match level.as_str() {
                                    "trace" => log::Level::Trace,
                                    "debug" => log::Level::Debug,
                                    "info" => log::Level::Info,
                                    "warn" => log::Level::Warn,
                                    "error" => log::Level::Error,
                                    _ => log::Level::Error,
                                },
                                None => log::Level::Info,
                            })
                            .file(file.as_deref())
                            .line(line)
                            .target("target")
                            .args(format_args!("{}{}", timestamp, frame.display_message()))
                            .build(),
                    );
                }
                Err(DecodeError::UnexpectedEof) => break,
                Err(DecodeError::Malformed) => {
                    match self.table.encoding().can_recover() {
                        // if recovery is impossible, abort
                        false => bail!("failed to decode defmt data"),
                        // if recovery is possible, skip the current frame and continue with new data
                        true => log::warn!("failed to decode defmt data"),
                    }
                }
            }
        }

        Ok(())
    }
}
