use std::{
    path::PathBuf,
    str::from_utf8,
    sync::atomic::{AtomicBool, Ordering},
};

use clap::Parser;
use eyre::{bail, ensure, Ok};
use futures::TryStreamExt;
use quick_xml::events::{BytesStart, Event};
use reqwest::Client;
use tokio::{
    fs::File,
    io::{AsyncBufRead, AsyncWriteExt, BufWriter},
};
use tokio_util::compat::FuturesAsyncReadCompatExt;

#[derive(Parser)]
struct Args {
    output: PathBuf,

    #[arg(long)]
    consonants: bool,
}

static CONSONANTS: AtomicBool = AtomicBool::new(false);

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    if args.consonants {
        CONSONANTS.store(true, Ordering::Relaxed);
    }

    let file = File::create(&args.output).await?;
    let mut writer = BufWriter::new(file);

    let client = Client::new();
    let books = ["Gen", "Exod", "Lev", "Num", "Deut"];

    let mut buf = Vec::new();
    for book in books {
        println!("Processing {book}...");
        let url = format!("https://raw.githubusercontent.com/bdenckla/MAM-XML/refs/heads/main/out/xml-vtrad-mam/{book}.xml");
        let response = client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .bytes_stream()
            .map_err(std::io::Error::other)
            .into_async_read()
            .compat();

        let mut xml = quick_xml::Reader::from_reader(response);

        {
            let config = xml.config_mut();
            config.trim_text_start = true;
        }

        assert_eq!(
            next(&mut xml, &mut buf).await?,
            Event::Start(
                BytesStart::new("book24").with_attributes([("versification-tradition", "vtmam")])
            )
        );

        assert_eq!(
            next(&mut xml, &mut buf).await?,
            Event::Start(BytesStart::new("book39").with_attributes([("osisID", book)]))
        );
        parse_chapters(&mut xml, &mut buf, &mut writer).await?;
    }
    Ok(())
}

async fn next<'buf>(
    xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
    buf: &'buf mut Vec<u8>,
) -> quick_xml::Result<Event<'buf>> {
    xml.read_event_into_async(buf).await
}

const IGNORE_TAGS: &[&[u8]] = &[b"spi-pe2", b"spi-samekh2", b"spi-samekh3"];

async fn parse_chapters(
    xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
    buf: &mut Vec<u8>,
    writer: &mut BufWriter<File>,
) -> eyre::Result<()> {
    loop {
        match next(xml, buf).await? {
            Event::Start(start) => match start.name().as_ref() {
                b"chapter" => continue,
                b"verse" => {
                    parse_complicated_verse(xml, buf, writer).await?;
                    if !CONSONANTS.load(Ordering::Relaxed) {
                        writer.write_all(b"\n").await?;
                    }
                }
                unknown => bail!("unknown tag {unknown:?}"),
            },
            Event::Empty(elem) if elem.name().as_ref() == b"verse" => {
                expect_text_attr(writer, elem).await?;
                if !CONSONANTS.load(Ordering::Relaxed) {
                    writer.write_all(b"\n").await?;
                }
            }
            Event::Empty(elem) if IGNORE_TAGS.contains(&elem.name().as_ref()) => {
                // TODO
                continue;
            }
            Event::End(_) => continue,
            Event::Eof => return Ok(()),
            unexpected => bail!("unexpected {unexpected:?}"),
        }
    }
}

async fn expect_text_attr(writer: &mut BufWriter<File>, elem: BytesStart<'_>) -> eyre::Result<()> {
    if !write_text_attr(writer, elem).await? {
        bail!("expected text");
    }
    Ok(())
}

async fn write_text_attr(
    writer: &mut BufWriter<File>,
    start: BytesStart<'_>,
) -> Result<bool, eyre::Error> {
    let char_buf = &mut [0; 2];
    let mut found_text = false;
    for attr in start.attributes() {
        let attr = attr?;
        if attr.key.as_ref() == b"text" {
            found_text = true;
            if CONSONANTS.load(Ordering::Relaxed) {
                for c in from_utf8(attr.value.as_ref())?.chars() {
                    if let ('\u{05d0}'..='\u{05EA}') = c {
                        c.encode_utf8(char_buf);
                        writer.write_all(char_buf).await?;
                    }
                }
            } else {
                writer.write_all(attr.value.as_ref()).await?;
            }
        }
    }
    Ok(found_text)
}

fn is_ignored_verse_tag(name: &[u8]) -> bool {
    const IGNORE_VERSE_TAGS: &[&[u8]] = &[b"lp-legarmeih", b"shirah-space", b"spi-invnun"];
    IGNORE_VERSE_TAGS.contains(&name) || IGNORE_TAGS.contains(&name)
}

async fn parse_complicated_verse(
    xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
    buf: &mut Vec<u8>,
    writer: &mut BufWriter<File>,
) -> eyre::Result<()> {
    loop {
        match next(xml, buf).await? {
            Event::Start(elem) => match elem.name().as_ref() {
                b"slh-word" => parse_slh_word(xml, buf, writer).await?,
                b"scrdfftar" => parse_scrdfftar(xml, buf, writer).await?,
                b"kq" => parse_kq(xml, buf, writer).await?,
                b"cant-all-three" => parse_cant_all_three(xml, buf, writer).await?,
                unexpected => bail!(
                    "unexpected {:?} at {}",
                    std::str::from_utf8(unexpected),
                    xml.buffer_position()
                ),
            },
            Event::Empty(elem) if elem.name().as_ref() == b"lp-paseq" => {
                if !CONSONANTS.load(Ordering::Relaxed) {
                    writer.write_all(b" \xD7\x80 ").await?;
                }
            }
            Event::Empty(elem) if elem.name().as_ref() == b"text" => {
                expect_text_attr(writer, elem).await?;
            }
            Event::Empty(elem) if elem.name().as_ref() == b"kq-trivial" => {
                expect_text_attr(writer, elem).await?;
            }
            Event::Empty(elem) if is_ignored_verse_tag(elem.name().as_ref()) => {
                // TODO
                continue;
            }
            Event::End(end) if end.name().as_ref() == b"verse" => break,
            unexpected => bail!("unexpected {unexpected:?}"),
        }
    }
    Ok(())
}

async fn parse_complicated_sdt(
    xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
    buf: &mut Vec<u8>,
    writer: &mut BufWriter<File>,
) -> eyre::Result<()> {
    loop {
        match next(xml, buf).await? {
            Event::Start(elem) => match elem.name().as_ref() {
                b"slh-word" => parse_slh_word(xml, buf, writer).await?,
                b"kq" => parse_kq(xml, buf, writer).await?,
                b"cant-all-three" => parse_cant_all_three(xml, buf, writer).await?,
                unexpected => bail!(
                    "unexpected {:?} at {}",
                    std::str::from_utf8(unexpected),
                    xml.buffer_position()
                ),
            },
            Event::Empty(elem) if elem.name().as_ref() == b"text" => {
                expect_text_attr(writer, elem).await?;
            }
            Event::Empty(elem) if elem.name().as_ref() == b"kq-trivial" => {
                expect_text_attr(writer, elem).await?;
            }
            Event::Empty(elem) if is_ignored_verse_tag(elem.name().as_ref()) => {
                // TODO
                continue;
            }
            Event::End(end) if end.name().as_ref() == b"sdt-target" => break,
            unexpected => bail!("unexpected {unexpected:?}"),
        }
    }
    Ok(())
}

async fn parse_complicated_cant(
    xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
    buf: &mut Vec<u8>,
    writer: &mut BufWriter<File>,
) -> eyre::Result<()> {
    loop {
        match next(xml, buf).await? {
            Event::Empty(elem) if elem.name().as_ref() == b"text" => {
                expect_text_attr(writer, elem).await?;
            }
            Event::Empty(elem) if is_ignored_verse_tag(elem.name().as_ref()) => {
                // TODO
                continue;
            }
            Event::End(end) if end.name().as_ref() == b"cant-alef" => {
                break;
            }
            unexpected => bail!("unexpected at {}: {unexpected:?}", xml.buffer_position()),
        }
    }
    Ok(())
}

async fn parse_kq(
    xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
    buf: &mut Vec<u8>,
    writer: &mut BufWriter<File>,
) -> eyre::Result<()> {
    let [mut found_q, mut found_k] = [false, false];
    loop {
        match next(xml, buf).await? {
            Event::Empty(elem) => match elem.name().as_ref() {
                b"kq-q" => {
                    if found_q {
                        bail!("found two q");
                    }
                    found_q = true;
                    expect_text_attr(writer, elem).await?;
                }
                b"kq-k" => {
                    if found_k {
                        bail!("found two k");
                    }
                    found_k = true;
                }
                unknown => bail!("unknown tag {unknown:?}"),
            },
            Event::End(end) if end.name().as_ref() == b"kq" => break,
            unexpected => bail!("unexpected {unexpected:?}"),
        }
    }
    ensure!(found_q, "found no q");
    ensure!(found_k, "found no k");
    Ok(())
}
async fn parse_cant_all_three(
    xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
    buf: &mut Vec<u8>,
    writer: &mut BufWriter<File>,
) -> eyre::Result<()> {
    match next(xml, buf).await? {
        Event::Empty(elem) if elem.name().as_ref() == b"cant-combined" => (),
        Event::Start(elem) if elem.name().as_ref() == b"cant-combined" => loop {
            if let Event::End(end) = next(xml, buf).await? {
                if end.name().as_ref() == b"cant-combined" {
                    break;
                }
            }
        },
        _ => bail!("expected combined"),
    };
    match next(xml, buf).await? {
        Event::Empty(elem) if elem.name().as_ref() == b"cant-alef" => {
            expect_text_attr(writer, elem).await?;
        }
        Event::Start(elem) if elem.name().as_ref() == b"cant-alef" => {
            parse_complicated_cant(xml, buf, writer).await?;
        }
        _ => bail!("expected alef"),
    };
    match next(xml, buf).await? {
        Event::Empty(elem) if elem.name().as_ref() == b"cant-bet" => (),
        Event::Start(elem) if elem.name().as_ref() == b"cant-bet" => loop {
            if let Event::End(end) = next(xml, buf).await? {
                if end.name().as_ref() == b"cant-bet" {
                    break;
                }
            }
        },
        _ => bail!("expected bet"),
    };

    {
        let Event::End(end) = next(xml, buf).await? else {
            bail!("expected end");
        };
        if end.name().as_ref() != b"cant-all-three" {
            bail!("expected all-three");
        }
    }
    Ok(())
}

async fn parse_scrdfftar(
    xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
    buf: &mut Vec<u8>,
    writer: &mut BufWriter<File>,
) -> eyre::Result<()> {
    match next(xml, buf).await? {
        Event::Start(elem) if elem.name().as_ref() == b"sdt-target" => {
            parse_complicated_sdt(xml, buf, writer).await?;
        }
        Event::Empty(elem) if elem.name().as_ref() == b"sdt-target" => {
            write_text_attr(writer, elem).await?;
        }
        e => bail!("expected sdt-target, found: {e:?}"),
    };
    loop {
        let Event::End(end) = next(xml, buf).await? else {
            continue;
        };
        if end.name().as_ref() == b"scrdfftar" {
            return Ok(());
        }
    }
}

async fn parse_slh_word(
    xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
    buf: &mut Vec<u8>,
    writer: &mut BufWriter<File>,
) -> eyre::Result<()> {
    loop {
        match next(xml, buf).await? {
            Event::Empty(elem) => match elem.name().as_ref() {
                b"text" | b"letter-large" | b"letter-small" => {
                    if !write_text_attr(writer, elem).await? {
                        bail!("expected text");
                    }
                }
                unknown => bail!("unknown tag {:?}", std::str::from_utf8(unknown)),
            },
            Event::End(end) if end.name().as_ref() == b"slh-word" => break,
            unexpected => bail!("unexpected {unexpected:?}"),
        }
    }
    Ok(())
}
