use std::{borrow::Cow, fmt::Display, path::PathBuf, str::from_utf8};

use clap::{Parser, ValueEnum};
use eyre::{bail, ensure, Ok};
use futures::TryStreamExt;
use phf::{phf_map, Map};
use quick_xml::events::{attributes::Attribute, BytesStart, Event};
use reqwest::Client;
use tokio::{
    fs::File,
    io::{AsyncBufRead, AsyncWriteExt, BufWriter},
};
use tokio_util::compat::FuturesAsyncReadCompatExt;

#[derive(Parser)]
struct Args {
    output: PathBuf,

    #[arg(short, long, default_value_t = Target::default())]
    target: Target,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, ValueEnum)]
enum Target {
    #[default]
    Teamim,
    Search,
    Training,
    Diff,
}

impl Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Teamim => f.write_str("teamim"),
            Target::Search => f.write_str("search"),
            Target::Training => f.write_str("training"),
            Target::Diff => f.write_str("diff"),
        }
    }
}

const COMMIT: &str = "6684604e161a4f7910fcac58d6dca2c07d21f6a4";

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    let file = File::create(&args.output).await?;
    let mut writer = BufWriter::new(file);

    let client = Client::new();
    let books = ["Gen", "Exod", "Lev", "Num", "Deut"];

    let mut buf = Vec::new();
    for book in books {
        println!("Processing {book}...");
        let url = format!("https://raw.githubusercontent.com/bdenckla/MAM-XML/{COMMIT}/out/xml-vtrad-mam/{book}.xml");
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

        let mut ctx = Context::new(args.target);

        assert_eq!(
            ctx.next(&mut xml, &mut buf).await?,
            Event::Start(
                BytesStart::new("book24").with_attributes([("versification-tradition", "vtmam")])
            )
        );

        assert_eq!(
            ctx.next(&mut xml, &mut buf).await?,
            Event::Start(BytesStart::new("book39").with_attributes([("osisID", book)]))
        );
        ctx.parse_chapters(&mut xml, &mut buf, &mut writer).await?;
    }
    Ok(())
}

pub static WIDE_LETTERS: Map<char, char> = phf_map! {
    'א'  =>'ﬡ',
    'ד'  =>'ﬢ',
    'ה'  =>'ﬣ',
    'כ'  =>'ﬤ',
    'ל'  =>'ﬥ',
    'ס'  =>'ﬦ',
    'ר'  =>'ﬧ',
    'ת'  =>'ﬨ',
};

struct Context {
    target: Target,
    wide_letter_interval: usize,
}

impl Context {
    fn new(target: Target) -> Self {
        Self {
            target,
            wide_letter_interval: 0,
        }
    }
}

const IGNORE_TAGS: &[&[u8]] = &[b"spi-pe2", b"spi-samekh2", b"spi-samekh3"];
fn is_ignored_verse_tag(name: &[u8]) -> bool {
    const IGNORE_VERSE_TAGS: &[&[u8]] = &[b"lp-legarmeih", b"shirah-space", b"spi-invnun"];
    IGNORE_VERSE_TAGS.contains(&name) || IGNORE_TAGS.contains(&name)
}

impl Context {
    async fn next<'buf>(
        &mut self,
        xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
        buf: &'buf mut Vec<u8>,
    ) -> quick_xml::Result<Event<'buf>> {
        xml.read_event_into_async(buf).await
    }

    async fn parse_chapters(
        &mut self,
        xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
        buf: &mut Vec<u8>,
        writer: &mut BufWriter<File>,
    ) -> eyre::Result<()> {
        loop {
            match self.next(xml, buf).await? {
                Event::Start(start) => match start.name().as_ref() {
                    b"chapter" => continue,
                    b"verse" => {
                        self.parse_complicated_verse(xml, buf, writer).await?;
                        if self.target == Target::Teamim {
                            writer.write_all(b"\n").await?;
                        }
                    }
                    unknown => bail!("unknown tag {unknown:?}"),
                },
                Event::Empty(elem) if elem.name().as_ref() == b"verse" => {
                    self.expect_text_attr(writer, elem).await?;
                    if self.target == Target::Teamim {
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

    async fn expect_text_attr(
        &mut self,
        writer: &mut BufWriter<File>,
        elem: BytesStart<'_>,
    ) -> eyre::Result<()> {
        let Some(text) = get_text_attr(&elem)? else {
            bail!("expected text");
        };
        self.write_text(writer, text.as_ref()).await?;
        Ok(())
    }

    async fn write_text(&mut self, writer: &mut BufWriter<File>, text: &[u8]) -> eyre::Result<()> {
        let char_buf = &mut [0; 4];
        match self.target {
            Target::Teamim => {
                writer.write_all(text).await?;
            }
            Target::Search => {
                for c in from_utf8(text)?.chars() {
                    if let ('\u{05d0}'..='\u{05EA}') = c {
                        c.encode_utf8(char_buf);
                        writer.write_all(char_buf).await?;
                    }
                }
            }
            Target::Training | Target::Diff => {
                for c in from_utf8(text)?.chars() {
                    match c {
                        '\u{05d0}'..='\u{05EA}' | ' ' => {
                            let c = WIDE_LETTERS
                                .get(&c)
                                .filter(|_| {
                                    if self.target == Target::Diff {
                                        return false;
                                    }
                                    if self.wide_letter_interval >= 25 {
                                        self.wide_letter_interval = 0;
                                        true
                                    } else {
                                        self.wide_letter_interval += 1;
                                        false
                                    }
                                })
                                .unwrap_or(&c)
                                .encode_utf8(char_buf);
                            writer.write_all(c.as_bytes()).await?;
                        }
                        // Sof Pasuq | Maqaf
                        '\u{05c3}' | '\u{05be}' => writer.write_all(b" ").await?,
                        _ => (),
                    }
                }
            }
        }

        Ok(())
    }

    async fn parse_complicated_verse(
        &mut self,
        xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
        buf: &mut Vec<u8>,
        writer: &mut BufWriter<File>,
    ) -> eyre::Result<()> {
        loop {
            match self.next(xml, buf).await? {
                Event::Start(elem) => match elem.name().as_ref() {
                    b"slh-word" => self.parse_slh_word(xml, buf, writer).await?,
                    b"scrdfftar" => self.parse_scrdfftar(xml, buf, writer).await?,
                    b"kq" => self.parse_kq(xml, buf, writer).await?,
                    b"cant-all-three" => self.parse_cant_all_three(xml, buf, writer).await?,
                    unexpected => bail!(
                        "unexpected {:?} at {}",
                        std::str::from_utf8(unexpected),
                        xml.buffer_position()
                    ),
                },
                Event::Empty(elem) if elem.name().as_ref() == b"lp-paseq" => match self.target {
                    Target::Teamim => {
                        writer.write_all(b" \xD7\x80 ").await?;
                    }
                    Target::Training | Target::Diff => writer.write_all(b" ").await?,
                    Target::Search => (),
                },
                Event::Empty(elem) if elem.name().as_ref() == b"text" => {
                    self.expect_text_attr(writer, elem).await?;
                }
                Event::Empty(elem) if elem.name().as_ref() == b"kq-trivial" => {
                    self.expect_text_attr(writer, elem).await?;
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
        &mut self,
        xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
        buf: &mut Vec<u8>,
        writer: &mut BufWriter<File>,
    ) -> eyre::Result<()> {
        loop {
            match self.next(xml, buf).await? {
                Event::Start(elem) => match elem.name().as_ref() {
                    b"slh-word" => self.parse_slh_word(xml, buf, writer).await?,
                    b"kq" => self.parse_kq(xml, buf, writer).await?,
                    b"cant-all-three" => self.parse_cant_all_three(xml, buf, writer).await?,
                    unexpected => bail!(
                        "unexpected {:?} at {}",
                        std::str::from_utf8(unexpected),
                        xml.buffer_position()
                    ),
                },
                Event::Empty(elem) if elem.name().as_ref() == b"text" => {
                    self.expect_text_attr(writer, elem).await?;
                }
                Event::Empty(elem) if elem.name().as_ref() == b"kq-trivial" => {
                    self.expect_text_attr(writer, elem).await?;
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
        &mut self,
        xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
        buf: &mut Vec<u8>,
        writer: &mut BufWriter<File>,
    ) -> eyre::Result<()> {
        loop {
            match self.next(xml, buf).await? {
                Event::Empty(elem) if elem.name().as_ref() == b"text" => {
                    self.expect_text_attr(writer, elem).await?;
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
        &mut self,
        xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
        buf: &mut Vec<u8>,
        writer: &mut BufWriter<File>,
    ) -> eyre::Result<()> {
        let [mut found_q, mut found_k] = [false, false];
        loop {
            match self.next(xml, buf).await? {
                Event::Empty(elem) => match elem.name().as_ref() {
                    b"kq-q" => {
                        if found_q {
                            bail!("found two q");
                        }
                        found_q = true;
                        self.expect_text_attr(writer, elem).await?;
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
        &mut self,
        xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
        buf: &mut Vec<u8>,
        writer: &mut BufWriter<File>,
    ) -> eyre::Result<()> {
        match self.next(xml, buf).await? {
            Event::Empty(elem) if elem.name().as_ref() == b"cant-combined" => (),
            Event::Start(elem) if elem.name().as_ref() == b"cant-combined" => loop {
                if let Event::End(end) = self.next(xml, buf).await? {
                    if end.name().as_ref() == b"cant-combined" {
                        break;
                    }
                }
            },
            _ => bail!("expected combined"),
        };
        match self.next(xml, buf).await? {
            Event::Empty(elem) if elem.name().as_ref() == b"cant-alef" => {
                self.expect_text_attr(writer, elem).await?;
            }
            Event::Start(elem) if elem.name().as_ref() == b"cant-alef" => {
                self.parse_complicated_cant(xml, buf, writer).await?;
            }
            _ => bail!("expected alef"),
        };
        match self.next(xml, buf).await? {
            Event::Empty(elem) if elem.name().as_ref() == b"cant-bet" => (),
            Event::Start(elem) if elem.name().as_ref() == b"cant-bet" => loop {
                if let Event::End(end) = self.next(xml, buf).await? {
                    if end.name().as_ref() == b"cant-bet" {
                        break;
                    }
                }
            },
            _ => bail!("expected bet"),
        };

        {
            let Event::End(end) = self.next(xml, buf).await? else {
                bail!("expected end");
            };
            if end.name().as_ref() != b"cant-all-three" {
                bail!("expected all-three");
            }
        }
        Ok(())
    }

    async fn parse_scrdfftar(
        &mut self,
        xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
        buf: &mut Vec<u8>,
        writer: &mut BufWriter<File>,
    ) -> eyre::Result<()> {
        match self.next(xml, buf).await? {
            Event::Start(elem) if elem.name().as_ref() == b"sdt-target" => {
                self.parse_complicated_sdt(xml, buf, writer).await?;
            }
            Event::Empty(elem) if elem.name().as_ref() == b"sdt-target" => {
                if let Some(text) = get_text_attr(&elem)? {
                    self.write_text(writer, &text).await?;
                }
            }
            e => bail!("expected sdt-target, found: {e:?}"),
        };
        loop {
            let Event::End(end) = self.next(xml, buf).await? else {
                continue;
            };
            if end.name().as_ref() == b"scrdfftar" {
                return Ok(());
            }
        }
    }

    async fn parse_slh_word(
        &mut self,
        xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
        buf: &mut Vec<u8>,
        writer: &mut BufWriter<File>,
    ) -> eyre::Result<()> {
        loop {
            match self.next(xml, buf).await? {
                Event::Empty(elem) => match elem.name().as_ref() {
                    b"text" | b"letter-large" | b"letter-small" => {
                        self.expect_text_attr(writer, elem).await?;
                    }

                    unknown => bail!("unknown tag {:?}", std::str::from_utf8(unknown)),
                },
                Event::End(end) if end.name().as_ref() == b"slh-word" => break,
                unexpected => bail!("unexpected {unexpected:?}"),
            }
        }
        Ok(())
    }
}

fn get_text_attr<'a>(elem: &'a BytesStart<'_>) -> eyre::Result<Option<Cow<'a, [u8]>>> {
    for attr in elem.attributes() {
        let attr: Attribute<'a> = attr?;
        if attr.key.as_ref() == b"text" {
            return Ok(Some(attr.value));
        }
    }
    Ok(None)
}
