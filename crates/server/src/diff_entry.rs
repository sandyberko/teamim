use eyre::OptionExt;
use maud::html;
use std::path::PathBuf;
use tokio::fs;

pub(crate) async fn diff_entry((i, line): (usize, &str)) -> eyre::Result<String> {
    let (_, url) = line.split_once('\t').ok_or_eyre("no separating tab")?;
    let has_save = fs::try_exists(
        PathBuf::from("assets/corrected-diffs")
            .join(url)
            .with_extension("html"),
    )
    .await?;
    let has_box = fs::try_exists(
        PathBuf::from("assets/corrected_boxfiles")
            .join(url.replace("/", "_"))
            .with_extension("box"),
    )
    .await?;
    Ok(html! {
        tr {
            td { (i) }
            td { a href={"#" (url)} { (url) } }
            td {
                (if has_save { "✅" } else { "❌" })
            }
            td {
                (if has_box { "✅" } else { "❌" })
            }
        }
    }
    .into_string())
}
