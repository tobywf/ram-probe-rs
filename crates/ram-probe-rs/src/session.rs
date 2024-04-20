use probe_rs::probe::list::Lister;
use probe_rs::probe::DebugProbeSelector;
use probe_rs::{Error, Permissions, Session, Target};

#[derive(Debug, Clone, clap::Parser)]
pub struct ProbeArgs {
    /// Chip name
    #[clap(long, env = "PROBE_RS_CHIP")]
    pub chip: String,

    /// Use this flag to select a specific probe in the list
    #[clap(long, env = "PROBE_RUN_PROBE", value_name = "PROBE_SELECTOR")]
    pub probe: Option<DebugProbeSelector>,

    /// The protocol speed in kHz
    #[clap(long)]
    pub speed: Option<u32>,

    /// Use this flag to assert the nreset & ntrst pins during attaching the probe to the chip
    #[clap(long)]
    pub connect_under_reset: bool,
}

/// Connect to a debug probe.
pub fn connect(args: &ProbeArgs, target: Target) -> Result<Session, Error> {
    let lister = Lister::new();

    let mut probe = match &args.probe {
        Some(selector) => {
            log::debug!("opening specified probe `{}`", selector);
            lister.open(selector)?
        }
        None => {
            let probes = lister.list_all();
            match &probes[..] {
                [] => return Err(Error::UnableToOpenProbe("no probe was found")),
                [info] => {
                    let selector = DebugProbeSelector::from(info);
                    log::debug!("opening default probe `{}`", selector);
                    info.open(&lister)?
                }
                _ => {
                    return Err(Error::UnableToOpenProbe(
                        "more than one probe found; use `--probe` to specify which one to use",
                    ))
                }
            }
        }
    };

    log::debug!("opened probe");

    if let Some(speed_khz) = args.speed {
        probe.set_speed(speed_khz)?;
    }

    let permissions = Permissions::new();

    let session = if args.connect_under_reset {
        probe.attach_under_reset(target, permissions)?
    } else {
        probe.attach(target, permissions)?
    };
    log::debug!("started session");

    Ok(session)
}
