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
                if matches!(entry.group(), Some(apob::ApobGroup::GENERAL))
                    && entry.ty == 6
                {
                    println!("    Milan APOB event log");
                    println!("    -------------------------------------");
                    println!("    INDEX   CLASS         EVENT  DATA");
                    let (log, _) =
                        apob::MilanApobEventLog::ref_from_prefix(&item.data)
                            .unwrap();
                    for (i, v) in
                        log.events[..log.count as usize].iter().enumerate()
                    {
                        println!(
                            "       {i:02x}  {:>12}  {:>6x}  {:#x} {:#x}",
                            if let Some(c) =
                                apob::MilanApobEventClass::from_repr(
                                    v.class as usize
                                )
                            {
                                format!("{c:?} ({:#x})", v.class)
                            } else {
                                format!("{:#x}", v.class)
                            },
                            v.info,
                            v.data0,
                            v.data1
                        );
                    }
                } else if matches!(entry.group(), Some(apob::ApobGroup::FABRIC))
                    && entry.ty == apob::ApobFabricType::SYS_MEM_MAP as u32
                {
                    let (map, holes) =
                        apob::ApobSysMemMap::ref_from_prefix(&item.data)
                            .unwrap();
                    println!("    APOB fabric");
                    println!("    high_phys: {:#10x}", map.high_phys);
                    println!("    -------------------------------------");
                    println!("            BASE        SIZE  TYPE");
                    let holes =
                        <[apob::ApobSysMemMapHole]>::ref_from_bytes(holes)
                            .unwrap();
                    for h in &holes[..map.hole_count as usize] {
                        println!(
                            "    0x{:0>10x}  0x{:0>8x}  {:#04x}",
                            h.base, h.size, h.ty
                        );
                    }
                }
            }
        }
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
