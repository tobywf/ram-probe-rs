use color_eyre::eyre::{bail, Context as _, OptionExt, Result};
use color_eyre::{Section as _, SectionExt as _};
use ram_probe_rs::defmt::DefmtInfo;
use ram_probe_rs::elf::Parser;
use ram_probe_rs::probe_rs::config::get_target_by_name;
use ram_probe_rs::run::{DefmtOpts, DefmtRunner};
use ram_probe_rs::session::{connect, ProbeArgs};

#[derive(Debug, Clone, clap::Parser)]
#[command(version = "1.0", about = "Flash and run an ELF program from RAM")]
struct Args {
    /// The path to the ELF file to flash and run from RAM
    path: String,

    #[clap(flatten)]
    probe: ProbeArgs,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    try_init_logging()?;

    use clap::Parser as _;
    let args = Args::parse();

    log::debug!("target `{}`", args.probe.chip);
    let target = get_target_by_name(&args.probe.chip)?;

    log::debug!("reading `{}`", args.path);
    let data = std::fs::read(&args.path)
        .wrap_err("failed to read ELF file")
        .with_section(|| args.path.clone().header("Path"))?;

    let elf = Parser::new(&data)?;

    if log::log_enabled!(log::Level::Trace) {
        use ram_probe_rs::elf::object::ObjectSection as _;

        for (name, addr) in elf.named_symbols() {
            log::trace!("ELF symbol `{}` at 0x{:08x}", name, addr);
        }
        for (name, section) in elf.named_sections() {
            log::trace!(
                "ELF section `{}` at 0x{:08x} ({} bytes)",
                name,
                section.address(),
                section.size()
            );
        }
    }

    let segments = elf.ram_loadable_segments(&target)?;
    let rtt_addr = elf.rtt_address().ok_or_eyre("RTT symbol not found")?;
    log::debug!("RTT address 0x{:08x}", rtt_addr);
    let vector_table = elf
        .vector_table()?
        .ok_or_eyre("vector table section not found")?;
    log::debug!("{:?}", vector_table);
    let defmt = DefmtInfo::new(&data)?.ok_or_eyre("defmt info not found")?;
    if defmt.is_missing_debug() {
        log::warn!("defmt locations empty, is the ELF compiled with `debug = 2`?");
    }
    let opts = DefmtOpts::with_defaults(&segments, rtt_addr, &vector_table, &defmt);

    let mut session = connect(&args.probe, target)?;
    let mut runner = DefmtRunner::new(&mut session, &opts)?;
    runner.run(&mut session)?;
    Ok(())
}

fn try_init_logging() -> Result<()> {
    let mut builder = pretty_env_logger::formatted_builder();
    match std::env::var("RUST_LOG") {
        Ok(filters) => {
            builder.parse_filters(&filters);
        }
        Err(std::env::VarError::NotPresent) => {
            builder.filter_level(log::LevelFilter::Warn);
            // app output
            builder.filter_module(module_path!(), log::LevelFilter::Info);
            // target output
            builder.filter_module("target", log::LevelFilter::Info);
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            bail!("`RUST_LOG` is not unicode");
        }
    }
    Ok(builder.try_init()?)
}
