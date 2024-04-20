use defmt_decoder::{Location, Table};
use eyre::{eyre, Context as _, Result};
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
