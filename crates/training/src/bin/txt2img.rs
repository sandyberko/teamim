use std::{
    env::set_current_dir,
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
    process::Command,
};

use eyre::Context;
use phf::{Map, phf_map};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

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

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    set_current_dir("../training")?;

    let tess_dir = training::tess_dir();

    let available_fonts = [
        ("Guttman Stam", false),
        ("Guttman Stam", true),
        ("Shlomo Stam", false),
        ("Stam Ashkenaz CLM Medium", false),
    ];

    let files = available_fonts
        .into_par_iter()
        .map(|(font, wide_letters)| -> eyre::Result<PathBuf> {
            let file_name = format!("{}{}", font, if wide_letters { "_wide" } else { "" });

            let log_path = PathBuf::from("../training/training/logs")
                .join(&file_name)
                .with_extension("log");

            let text_file = if wide_letters {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../assets/text/mam/training-wide-letters.txt")
            } else {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/text/mam/training.txt")
            };

            let image_path = PathBuf::from("training/images").join(&file_name);

            // Create image and box files
            Command::new(tess_dir.join("text2image.exe"))
                .args(["--fonts_dir", "fonts"])
                .args(["--fontconfig_tmpdir", "tmp"])
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
            let lstmf_path = PathBuf::from("training/combined").join(&file_name);
            Command::new(tess_dir.join("tesseract.exe"))
                .arg(image_path.with_extension("tif"))
                .arg(&lstmf_path)
                .args(["--psm", "6"])
                .arg("lstm.train")
                .stderr(File::options().append(true).create(true).open(&log_path)?)
                .spawn()?
                .wait()?;

            Ok(lstmf_path.with_extension("lstmf"))
        })
        .collect::<eyre::Result<Vec<_>>>()?;

    // create list file
    let mut list_file = BufWriter::new(File::create("training/list.txt")?);
    for file in files {
        writeln!(list_file, "{}", file.display())?;
    }

    Ok(())
}
