mod types;

use eyre::{bail, eyre, Result};
pub use object;
use object::elf::{FileHeader32, PT_LOAD};
use object::read::elf::{ElfFile32, ElfSection32, FileHeader as _, ProgramHeader as _};
use object::read::Object as _;
pub use object::read::ObjectSection;
use object::{FileKind, LittleEndian, ObjectSymbol as _};
use probe_rs::config::MemoryRange as _;
use probe_rs::config::MemoryRegion;
use probe_rs::Target;
use std::convert::TryInto;
pub use types::*;

pub type ElfSection<'data, 'file> = ElfSection32<'data, 'file, LittleEndian>;

pub struct Parser<'data> {
    data: &'data [u8],
    header: FileHeader32<LittleEndian>,
    file: ElfFile32<'data, LittleEndian>,
}

impl<'data> Parser<'data> {
    pub fn new(data: &'data [u8]) -> Result<Self> {
        let file_kind = FileKind::parse(data)?;
        if !matches!(file_kind, FileKind::Elf32) {
            bail!("unsupported file type {:?}", file_kind);
        }
        let header = *FileHeader32::<LittleEndian>::parse(data)?;
        let file = ElfFile32::<LittleEndian>::parse(data)?;
        Ok(Self { data, header, file })
    }

    pub fn ram_loadable_segments(&self, target: &Target) -> Result<Segments<'data>> {
        let endian = LittleEndian;
        let mut loadable_segments = Vec::new();

        for segment in self.header.program_headers(endian, self.data)? {
            // Ignore non-loadable segments
            if segment.p_type(endian) != PT_LOAD {
                continue;
            }

            // Get the physical address of the segment. The data will be programmed to that location
            let paddr: u64 = segment.p_paddr(endian).into();

            let segment_data = segment
                .data(endian, self.data)
                .map_err(|()| eyre!("failed to read ELF segment {:?}", segment))?;

            // Ignore empty segments
            if segment_data.is_empty() {
                log::info!("segment at 0x{:08x} is empty, skipping", paddr);
                continue;
            }

            log::trace!("found loadable segment, physical address 0x{:08x}", paddr);

            let (segment_offset, segment_filesize) = segment.file_range(endian);
            let segment_range = segment_offset..segment_offset + segment_filesize;

            let mut matched = false;
            for section in self.file.sections() {
                let (section_offset, section_filesize) = match section.file_range() {
                    Some(range) => range,
                    None => continue,
                };
                let section_range = section_offset..section_offset + section_filesize;

                if segment_range.contains_range(&section_range) {
                    log::trace!(
                        "matched section `{}`",
                        section.name().unwrap_or("<non UTF-8 name>")
                    );
                    matched = true;
                }
            }

            if matched {
                log::debug!(
                    "matched segment at 0x{:08x} ({} bytes)",
                    paddr,
                    segment_filesize
                );

                // Check that the section is in RAM
                let start = paddr;
                let end = start + segment_filesize;

                let all_in_ram = target.memory_map.iter().any(|region| match region {
                    MemoryRegion::Ram(r) => {
                        let start_in_ram = r.range.start <= start;
                        let end_in_ram = r.range.end >= end;
                        start_in_ram && end_in_ram
                    }
                    MemoryRegion::Generic(_) => false,
                    MemoryRegion::Nvm(_) => false,
                });
                if !all_in_ram {
                    log::warn!("segment at 0x{:08x} is not in RAM", paddr);
                    bail!("ELF contains non-RAM data");
                }

                let section_data =
                    &self.data[segment_range.start as usize..segment_range.end as usize];
                loadable_segments.push((paddr, section_data));
            } else {
                log::warn!(
                    "segment at 0x{:08x} with no matching sections, skipping",
                    paddr
                );
            }
        }

        loadable_segments.sort_by_key(|(addr, _)| *addr);
        Ok(Segments(loadable_segments))
    }

    pub fn named_symbols(&self) -> impl Iterator<Item = (&'data str, u32)> + '_ {
        self.file.symbols().filter_map(|symbol| {
            symbol.name().ok().map(|name| {
                let address = symbol.raw_symbol().st_value.get(LittleEndian);
                (name, address)
            })
        })
    }

    pub fn named_sections(&self) -> impl Iterator<Item = (&'data str, ElfSection<'data, '_>)> + '_ {
        self.file
            .sections()
            .filter_map(|section| section.name().ok().map(|name| (name, section)))
    }

    pub fn rtt_address(&self) -> Option<u32> {
        self.named_symbols().find_map(|(name, address)| {
            if name == "_SEGGER_RTT" {
                Some(address)
            } else {
                None
            }
        })
    }

    pub fn vector_table(&self) -> Result<Option<VectorTable>> {
        self.named_sections()
            .find_map(|(name, section)| {
                if name == ".vector_table" {
                    Some(parse_vector_table(section))
                } else {
                    None
                }
            })
            .transpose()
    }
}

pub fn parse_vector_table(section: ElfSection32<'_, '_, LittleEndian>) -> Result<VectorTable> {
    let address = section.address() as u32;
    let size = section.size() as u32;

    if size < 3 * 4 {
        bail!("vector table section is too small");
    }

    let data = section.data()?;
    assert!(
        data.len() >= 3 * 4,
        "data length {} is smaller than size {}",
        data.len(),
        size
    );
    // Entry 0: initial stack pointer
    let initial_sp = u32::from_le_bytes(data[0..4].try_into().unwrap());
    // Entry 1: reset handler
    let reset = u32::from_le_bytes(data[4..8].try_into().unwrap());
    // Entry 2:  hard fault handler
    let hard_fault = u32::from_le_bytes(data[8..12].try_into().unwrap());

    Ok(VectorTable {
        address,
        initial_sp,
        reset,
        hard_fault,
    })
}
