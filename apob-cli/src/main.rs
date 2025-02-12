use anyhow::{Context, Result};
use clap::Parser;
use std::{
    io::{Read, Write},
    path::PathBuf,
};
use zerocopy::FromBytes;

/// Simple CLI to investigate an APOB file
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Prints data contents of all sections
    #[clap(short, long)]
    verbose: bool,
    /// Name of the file to load
    name: PathBuf,
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

    println!("{header:?}");
    println!(
        "{:<7}   {:<8}   {:>4}   {:>8}   {:>9}",
        "OFFSET", "GROUP", "TYPE", "INSTANCE", "DATA SIZE"
    );
    let mut pos = header.offset as usize;
    while pos < data.len() {
        let (entry, _rest) =
            apob::ApobEntry::ref_from_prefix(&data[pos..]).unwrap();

        println!(
            "{pos:#07x}   {:<8}   {:>4x}   {:>8x}   {:>9x}",
            format!("{:?}", entry.group().unwrap()),
            entry.ty & !apob::APOB_CANCELLED,
            entry.inst,
            entry.size as usize - std::mem::size_of_val(entry)
        );
        if args.verbose {
            print_hex(
                &mut std::io::stdout(),
                &data[pos..][..entry.size as usize]
                    [std::mem::size_of_val(entry)..],
            )
            .unwrap();
        }
        pos += entry.size as usize;
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
