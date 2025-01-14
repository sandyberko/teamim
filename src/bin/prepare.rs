use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
};

use clap::Parser;

#[derive(Parser)]
struct Args {
    input_dir: PathBuf,
    #[arg(short, long)]
    remove_paragraph_markers: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let out = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open("torah.txt")?;

    let mut writer = BufWriter::new(out);

    let mut buf = String::new();
    for in_file in fs::read_dir(args.input_dir)? {
        let in_file = in_file?;

        println!("Processing {:?}...", in_file.file_name());

        if !in_file.file_type()?.is_file() {
            println!("{:?} is not a file", in_file.file_name());
            continue;
        }
        let mut reader = BufReader::new(File::open(in_file.path())?);

        while {
            buf.clear();
            reader.read_line(&mut buf)? > 0
        } {
            if buf.starts_with("\u{202A}xxxx") {
                continue;
            }

            const START: usize = "\u{202b}\u{a0}\u{a0}".len();
            const END: usize = " \u{202c}\r\n".len();

            let line = &buf[START..buf.len() - END];

            let line = if args.remove_paragraph_markers {
                line.trim_end_matches(" פ").trim_end_matches(" ס")
            } else {
                line
            };

            writer.write_all(line.as_bytes())?;
            writeln!(writer)?;
        }
    }
    writer.flush()?;

    Ok(())
}
