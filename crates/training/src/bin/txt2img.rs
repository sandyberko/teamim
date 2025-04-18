use std::{
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    process::Command,
};

use clap::Parser;
use eyre::{Context, OptionExt};
use phf::{Map, phf_map};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use training::tess_dir;

static WIDE_LETTERS: Map<char, char> = phf_map! {
    'ﬡ' =>'א',
    'ﬢ' =>'ד',
    'ﬣ' =>'ה',
    'ﬤ' =>'כ',
    'ﬥ' =>'ל',
    'ﬦ' =>'ס',
    'ﬧ' =>'ר',
    'ﬨ' =>'ת',
};

#[derive(Debug, Parser)]
struct Args {
    txt_path: Option<PathBuf>,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    if let Some(txt_path) = args.txt_path {
        let txt_path = txt_path.canonicalize()?;
        let _lsmtf_path = process_txt("Guttman Stam", false, &txt_path)?;
        return Ok(());
    }

    let available_fonts = [
        ("Guttman Stam", false),
        ("Guttman Stam", true),
        ("Shlomo Stam", false),
        ("Stam Ashkenaz CLM Medium", false),
    ];

    let wide_txt_path = PathBuf::from("assets/text/mam/training-wide-letters.txt");
    let txt_path = PathBuf::from("assets/text/mam/training.txt");

    let files = available_fonts
        .into_par_iter()
        .map(|(font, wide_letters)| -> eyre::Result<PathBuf> {
            let path = if wide_letters {
                &wide_txt_path
            } else {
                &txt_path
            };

            process_txt(font, wide_letters, path)
        })
        .collect::<eyre::Result<Vec<_>>>()?;

    // create list file
    let mut list_file = BufWriter::new(File::create("../training/training/list.txt")?);
    for file in files {
        writeln!(list_file, "{}", file.display())?;
    }

    Ok(())
}

fn process_txt(font: &str, wide_letters: bool, text_file: &Path) -> eyre::Result<PathBuf> {
    let file_name = format!(
        "{stem}_{font}",
        stem = text_file
            .file_stem()
            .ok_or_eyre("no stem")?
            .to_string_lossy(),
    );

    let log_path = PathBuf::from("../training/training/logs")
        .join(&file_name)
        .with_extension("log");

    let image_path = PathBuf::from("../training/training/images").join(&file_name);

    // Create image and box files
    Command::new(tess_dir().join("text2image.exe"))
        .args(["--fonts_dir", "../training/fonts"])
        .args(["--fontconfig_tmpdir", "../training/tmp"])
        .args(["--font", font])
        .args(["--text".as_ref(), text_file.as_os_str()])
        .args(["--outputbase".as_ref(), image_path.as_os_str()])
        .args(["--max_pages", "0"])
        .args(["--resolution", "300"])
        .args(["--xsize", "2257"])
        .args(["--ysize", "5075"])
        .args(["--margin", "250"])
        .args(["--ptsize", "21"])
        .args(["--leading", "-38"])
        .args(["--distort_image", "true"])
        .stderr(File::create(&log_path)?)
        .spawn()?
        .wait()?;

    if wide_letters {
        let src_box = image_path.with_extension("wide-box");
        let dest_box = image_path.with_extension("box");
        fs::rename(&dest_box, &src_box).wrap_err_with(|| {
            format!(
                "failed to rename {dest_box:?} to {src_box:?}",
                dest_box = dest_box.display(),
                src_box = src_box.display()
            )
        })?;
        let mut r = BufReader::new(File::open(&src_box)?);
        let mut w = BufWriter::new(File::create(&dest_box)?);
        let mut buf = String::new();
        while r.read_line(&mut buf)? > 0 {
            let mut chars = buf.chars();
            if let Some(c) = chars.next().and_then(|c| WIDE_LETTERS.get(&c)) {
                write!(w, "{c}{}", chars.as_str())?;
            } else {
                w.write_all(buf.as_bytes())?;
            }
            buf.clear();
        }
    }

    // combine image and box into `.lstmf`
    let lstmf_path = PathBuf::from("../training/training/combined").join(&file_name);
    Command::new(tess_dir().join("tesseract.exe"))
        .arg(image_path.with_extension("tif"))
        .arg(&lstmf_path)
        .args(["--psm", "6"])
        .arg("lstm.train")
        .stderr(File::options().append(true).create(true).open(&log_path)?)
        .spawn()?
        .wait()?;

    Ok(lstmf_path.with_extension("lstmf"))
}
