use anyhow::{Context, Result};
use clap::Parser;
use std::{
    io::{Read, Write},
    path::PathBuf,
};
use zerocopy::FromBytes;

mod app;

/// Simple CLI to investigate an APOB file
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Prints raw data contents of all sections
    #[clap(short, long)]
    raw: bool,
    /// Decodes known section types
    #[clap(short, long)]
    decode: bool,
    /// Runs an interactive viewer
    #[clap(short, long)]
    interactive: bool,
    /// Name of the file to load
    name: PathBuf,
}

#[derive(Copy, Clone, Debug)]
enum Item {
    Header(apob::ApobHeader),
    Padding,
    Entry(apob::ApobEntry),
}

struct Entry {
    offset: usize,
    entry: Item,
    data: Vec<u8>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut f = std::fs::File::open(&args.name)
        .with_context(|| format!("failed to open {:?}", args.name))?;
    let mut data = vec![];
    f.read_to_end(&mut data).context("failed to read file")?;

    let (header, _rest) = apob::ApobHeader::ref_from_prefix(&data).unwrap();
    assert_eq!(header.sig, apob::APOB_SIG, "invalid signature");
    assert_eq!(header.version, apob::APOB_VERSION, "invalid version");

    let header_size = std::mem::size_of_val(header);
    let mut entries = vec![
        Entry {
            offset: 0,
            entry: Item::Header(*header),
            data: data[..header_size].to_owned(),
        },
        Entry {
            offset: header_size,
            entry: Item::Padding,
            data: data[header_size..header.offset as usize].to_owned(),
        },
    ];
    let mut pos = header.offset as usize;
    while pos < data.len() {
        let (entry, _rest) =
            apob::ApobEntry::ref_from_prefix(&data[pos..]).unwrap();
        let entry_data =
            &data[pos..][..entry.size as usize][std::mem::size_of_val(entry)..];
        entries.push(Entry {
            offset: pos,
            entry: Item::Entry(*entry),
            data: entry_data.to_vec(),
        });
        pos += entry.size as usize;
    }

    if args.interactive {
        let terminal = ratatui::init();
        let app = app::App::new(entries);
        app.run(terminal);
        ratatui::restore();
    } else {
        println!("{header:?}");
        println!(
            "{:<7}   {:<8}   {:>4}   {:>8}   {:>9}",
            "OFFSET", "GROUP", "TYPE", "INSTANCE", "DATA SIZE"
        );
        for item in &entries {
            let Item::Entry(entry) = &item.entry else {
                continue;
            };
            println!(
                "{:#07x}   {:<8}   {:>4x}   {:>8x}   {:>9x}",
                item.offset,
                format!("{:?}", entry.group().unwrap()),
                entry.ty & !apob::APOB_CANCELLED,
                entry.inst,
                entry.size as usize - std::mem::size_of_val(entry)
            );
            if args.raw {
                print_hex(&mut std::io::stdout(), &item.data).unwrap();
            }
            if args.decode {
                decode_item(&mut std::io::stdout(), entry, &item.data).unwrap();
            }
        }
    }

    Ok(())
}

fn decode_item<W: Write>(
    out: &mut W,
    entry: &apob::ApobEntry,
    data: &[u8],
) -> Result<(), std::io::Error> {
    let Some(group) = entry.group() else {
        return Ok(());
    };
    match (group, entry.ty) {
        (apob::ApobGroup::GENERAL, ty)
            if ty == apob::ApobGeneralType::EVENT_LOG as u32 =>
        {
            writeln!(out, "    Milan APOB event log")?;
            writeln!(out, "    -------------------------------------")?;
            writeln!(
                out,
                "    INDEX   CLASS        EVENT                 DATA"
            )?;
            let (log, _) =
                apob::MilanApobEventLog::ref_from_prefix(data).unwrap();
            for (i, v) in log.events[..log.count as usize].iter().enumerate() {
                writeln!(
                    out,
                    "       {i:02x}  {:>12}  {:<20}  {:#x} {:#x}",
                    if let Some(c) =
                        apob::MilanApobEventClass::from_repr(v.class as usize)
                    {
                        format!("{c:?} ({:#x})", v.class)
                    } else {
                        format!("{:#x}", v.class)
                    },
                    if let Some(c) =
                        apob::MilanApobEventInfo::from_repr(v.info as usize)
                    {
                        format!("{c:?} ({:#x})", v.info)
                    } else {
                        format!("{:#x}", v.info)
                    },
                    v.data0,
                    v.data1
                )?;
            }
        }
        (apob::ApobGroup::FABRIC, ty)
            if ty == apob::ApobFabricType::SYS_MEM_MAP as u32 =>
        {
            let (map, holes) =
                apob::ApobSysMemMap::ref_from_prefix(data).unwrap();
            writeln!(out, "    APOB fabric")?;
            writeln!(out, "    high_phys: {:#10x}", map.high_phys)?;
            writeln!(out, "    -------------------------------------")?;
            writeln!(out, "            BASE        SIZE  TYPE")?;
            let holes =
                <[apob::ApobSysMemMapHole]>::ref_from_bytes(holes).unwrap();
            for h in &holes[..map.hole_count as usize] {
                writeln!(
                    out,
                    "    0x{:0>10x}  0x{:0>8x}  {:#04x}",
                    h.base, h.size, h.ty
                )?;
            }
        }
        (apob::ApobGroup::MEMORY, ty)
            if ty == apob::ApobMemoryType::MILAN_PMU_TRAIN_FAIL as u32 =>
        {
            let (p, _) = apob::PmuTfi::ref_from_prefix(data).unwrap();
            writeln!(out, "    PMU training failure log")?;
            writeln!(out, "    -------------------------------------")?;
            writeln!(
                out,
                "    INDEX  SOCK UMC   1D2D 1DNUM  STAGE  ERROR   DATA"
            )?;
            for (i, h) in p.entries[..p.nvalid as usize].iter().enumerate() {
                writeln!(
                    out,
                    "       {i:02x}  {:>4} {:>3}  {:>5} {:>5} {:>6}  {:x}  {:x} {:x} {:x} {:x}",
                    h.bits.sock(),
                    h.bits.umc(),
                    h.bits.dimension(),
                    h.bits.num_1d(),
                    h.bits.stage(),
                    h.error,
                    h.data[0],
                    h.data[1],
                    h.data[2],
                    h.data[3],
                )?;
            }
        }
        _ => (),
    }
    Ok(())
}

fn print_hex<W: Write>(out: &mut W, data: &[u8]) -> Result<(), std::io::Error> {
    writeln!(
        out,
        "            00 01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f"
    )?;
    let mut addr = 0;
    for d in data.chunks(16) {
        write!(out, "    {addr:04x} |  ")?;
        for c in d {
            write!(out, "{c:02x} ")?;
        }
        for _ in 0..16 - d.len() {
            write!(out, "   ")?;
        }
        write!(out, "| ")?;
        for &c in d {
            if c.is_ascii() && !c.is_ascii_control() {
                write!(out, "{}", c as char)?;
            } else {
                write!(out, ".")?;
            }
        }
        writeln!(out)?;

        addr += d.len();
    }
    Ok(())
}
