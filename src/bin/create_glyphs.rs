use std::process::Command;

use teamim::glyph::GLYPHS;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let results = GLYPHS
        .entries()
        .map(|(&char, glyph)| {
            Command::new("ffmpeg")
                .arg("-hide_banner")
                .args(["-loglevel", "error"])
                .arg("-y")
                .args(["-f", "lavfi"])
                .args([
                    "-i",
                    &format!(
                        "color=color=white:\
                        s=64x64,format=rgba,drawtext=text='{char}':\
                        font='Guttman Stam':ft_load_flags=0:\
                        fontcolor=black:\
                        fontsize=64:\
                        x=10:y=10",
                    ),
                ])
                .args(["-frames:v", "1"])
                .args(["-update", "1"])
                .arg(format!("assets/glyphs/{}.tif", glyph.name))
                .spawn()
        })
        .collect::<Result<Vec<_>, _>>()?;

    for mut result in results {
        result.wait()?;
    }
    Ok(())
}
