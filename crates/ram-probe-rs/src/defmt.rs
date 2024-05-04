pub use defmt_decoder;
use defmt_decoder::{DecodeError, Location, StreamDecoder, Table};
use eyre::{bail, eyre, Context as _, Result};
use std::collections::BTreeMap;
use std::fmt;

pub struct DefmtInfo {
    pub table: Box<Table>,
    pub locations: BTreeMap<u64, Location>,
}

impl DefmtInfo {
    pub fn new(data: &[u8]) -> Result<Option<DefmtInfo>> {
        log::debug!("parsing defmt table");
        let table = defmt_decoder::Table::parse(data)
            .map_err(|err| eyre!(Box::new(err)))
            .wrap_err("failed to parse defmt table from ELF file")?;

        match table {
            None => Ok(None),
            Some(table) => {
                let table = Box::new(table);

                log::debug!("parsing defmt locations");
                let locations = table
                    .get_locations(data)
                    .map_err(|err| eyre!(Box::new(err)))
                    .wrap_err("failed to parse defmt locations from ELF file")?;

                Ok(Some(DefmtInfo { table, locations }))
            }
        }
    }

    /// Returns true if the ELF file was likely compiled without `debug = 2`.
    pub fn is_missing_debug(&self) -> bool {
        !self.table.is_empty() && self.locations.is_empty()
    }
}

impl fmt::Debug for DefmtInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefmtInfo")
            .field("table", &"...")
            .field("locations", &"...")
            .finish()
    }
}

pub struct DefmtDecoder<'opts> {
    stream: Box<dyn StreamDecoder + 'opts>,
    table: &'opts Table,
    locations: &'opts BTreeMap<u64, Location>,
    target: &'opts str,
}

impl<'opts> DefmtDecoder<'opts> {
    pub fn new(opts: &'opts DefmtInfo, target: &'opts str) -> Self {
        let DefmtInfo { table, locations } = &opts;
        let stream = table.new_stream_decoder();
        Self {
            stream,
            table,
            locations,
            target,
        }
    }
}

impl DefmtDecoder<'_> {
    pub fn decode(&mut self, data: &[u8]) -> Result<()> {
        self.stream.received(data);

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
                            .target(self.target)
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
