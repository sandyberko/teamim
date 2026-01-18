use std::{borrow::Cow, fmt::Display, path::PathBuf, str::from_utf8};

use clap::{Parser, ValueEnum};
use eyre::{bail, ensure};
use futures::TryStreamExt;
use phf::{Map, phf_map};
use quick_xml::events::{BytesStart, Event, attributes::Attribute};
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
    /// With teamim, sof-pasuq, maqaf, and verse per line
    Teamim,
    /// Consonants only, no spaces, single line
    Search,
    /// Consonants only, with spaces, single line
    Training,
    /// Same as [`Target::Training`], but with wide letters inserted every [`WIDE_LETTER_INTERVAL`]
    TrainingWideLetters,
}

impl Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Teamim => f.write_str("teamim"),
            Target::Search => f.write_str("search"),
            Target::TrainingWideLetters => f.write_str("training"),
            Target::Training => f.write_str("diff"),
        }
    }
}

const COMMIT: &str = "fcd85bc74d3688f5b28dd2158e2c1168c51d4e72";
const WIDE_LETTER_INTERVAL: usize = 25;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    let file = File::create(&args.output).await?;
    let mut writer = BufWriter::new(file);

    let client = Client::new();
    let books = ["Gen", "Exod", "Lev", "Num", "Deut", "Esth"];

    let mut buf = Vec::new();
    for book in books {
        println!("Processing {book}...");
        let url = format!(
            "https://raw.githubusercontent.com/bdenckla/MAM-XML/{COMMIT}/out/xml-vtrad-mam/{book}.xml"
        );
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
    writer.flush().await?;
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

const IGNORED_TAG_PREFIX: &[u8] = b"spi-";

struct Context {
    target: Target,
    wide_letter_elapsed: usize,
}

impl Context {
    fn new(target: Target) -> Self {
        Self { target, wide_letter_elapsed: 0 }
    }
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
                    }
                    unknown => bail!("unknown tag {unknown:?}"),
                },
                Event::Empty(elem) if elem.name().as_ref() == b"verse" => {
                    self.expect_text_attr(writer, elem).await?;
                    match self.target {
                        Target::Teamim => {
                            writer.write_all(b"\n").await?;
                        }
                        Target::TrainingWideLetters | Target::Training => {
                            writer.write_all(b" ").await?;
                        }
                        Target::Search => (),
                    }
                }
                Event::Empty(elem) if elem.name().as_ref().starts_with(IGNORED_TAG_PREFIX) => {
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
                        writer.write_all(c.encode_utf8(char_buf).as_bytes()).await?;
                    }
                }
            }
            Target::TrainingWideLetters | Target::Training => {
                for c in from_utf8(text)?.chars() {
                    match c {
                        '\u{05d0}'..='\u{05EA}' | ' ' => {
                            let c = WIDE_LETTERS
                                .get(&c)
                                .filter(|_| {
                                    if self.target == Target::Training {
                                        return false;
                                    }
                                    if self.wide_letter_elapsed >= WIDE_LETTER_INTERVAL {
                                        self.wide_letter_elapsed = 0;
                                        true
                                    } else {
                                        self.wide_letter_elapsed += 1;
                                        false
                                    }
                                })
                                .unwrap_or(&c)
                                .encode_utf8(char_buf);
                            writer.write_all(c.as_bytes()).await?;
                        }
                        // Sof Pasuq
                        '\u{05c3}' => continue,
                        // Maqaf
                        '\u{05be}' => writer.write_all(b" ").await?,
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
        let mut trailing_shirah_space = false;
        loop {
            let event = self.next(xml, buf).await?;

            // handle spaces
            match event {
                Event::Empty(elem) if elem.name().as_ref() == b"shirah-space" => {
                    trailing_shirah_space = true;
                    if self.target != Target::Search {
                        writer.write_all(b" ").await?;
                    }
                }
                Event::End(end) if end.name().as_ref() == b"verse" => {
                    match self.target {
                        Target::Teamim => {
                            writer.write_all(b"\n").await?;
                        }
                        Target::Training | Target::TrainingWideLetters => {
                            if !trailing_shirah_space {
                                writer.write_all(b" ").await?;
                            }
                        }
                        Target::Search => (),
                    }
                    return Ok(());
                }

                // handle other tags
                event => {
                    trailing_shirah_space = false;

                    match self.sof_pasuq(&event, writer).await {
                        Some(Ok(())) => continue,
                        Some(Err(err)) => return Err(err),
                        None => (),
                    }

                    match event {
                        Event::Start(elem) => match elem.name().as_ref() {
                            b"slh-word" => self.parse_slh_word(xml, buf, writer).await?,
                            b"scrdfftar" => self.parse_scrdfftar(xml, buf, writer).await?,
                            b"kq" => self.parse_kq(xml, buf, writer).await?,
                            b"cant-all-three" => {
                                self.parse_cant_all_three(xml, buf, writer).await?;
                            }
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
                        Event::Empty(elem)
                            if elem.name().as_ref().starts_with(IGNORED_TAG_PREFIX) =>
                        {
                            // TODO
                            continue;
                        }
                        unexpected => bail!("unexpected {unexpected:?}"),
                    }
                }
            }
        }
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
                Event::Empty(elem) if elem.name().as_ref().starts_with(IGNORED_TAG_PREFIX) => {
                    // TODO
                    continue;
                }
                Event::End(end) if end.name().as_ref() == b"sdt-target" => break,
                unexpected => bail!("unexpected {unexpected:?}"),
            }
        }
        Ok(())
    }

    async fn sof_pasuq(
        &mut self,
        event: &Event<'_>,
        writer: &mut BufWriter<File>,
    ) -> Option<eyre::Result<()>> {
        const SOF_PASUQ_TAGS: &[&[u8]] = &[b"lp-paseq", b"lp-legarmeih", b"lp-legarmeih "];

        let Event::Empty(tag) = event else { return None };

        if !SOF_PASUQ_TAGS.contains(&tag.name().as_ref()) {
            return None;
        }

        Some(match self.target {
            Target::Teamim => writer.write_all(b" \xD7\x80 ").await.map_err(eyre::Error::from),
            Target::TrainingWideLetters | Target::Training => {
                writer.write_all(b" ").await.map_err(eyre::Error::from)
            }
            Target::Search => Ok(()),
        })
    }
    async fn parse_complicated_cant(
        &mut self,
        xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
        buf: &mut Vec<u8>,
        writer: &mut BufWriter<File>,
    ) -> eyre::Result<()> {
        loop {
            let event = self.next(xml, buf).await?;

            match self.sof_pasuq(&event, writer).await {
                Some(Ok(())) => continue,
                Some(Err(err)) => return Err(err),
                None => (),
            }

            match event {
                Event::Empty(elem) if elem.name().as_ref() == b"text" => {
                    self.expect_text_attr(writer, elem).await?;
                }
                Event::Empty(elem) if elem.name().as_ref().starts_with(IGNORED_TAG_PREFIX) => {
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
                    }
                    b"kq-k" => {
                        if found_k {
                            bail!("found two k");
                        }
                        found_k = true;
                        self.expect_text_attr(writer, elem).await?;
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
                if let Event::End(end) = self.next(xml, buf).await?
                    && end.name().as_ref() == b"cant-combined"
                {
                    break;
                }
            },
            _ => bail!("expected combined"),
        }
        match self.next(xml, buf).await? {
            Event::Empty(elem) if elem.name().as_ref() == b"cant-alef" => {
                self.expect_text_attr(writer, elem).await?;
            }
            Event::Start(elem) if elem.name().as_ref() == b"cant-alef" => {
                self.parse_complicated_cant(xml, buf, writer).await?;
            }
            _ => bail!("expected alef"),
        }
        match self.next(xml, buf).await? {
            Event::Empty(elem) if elem.name().as_ref() == b"cant-bet" => (),
            Event::Start(elem) if elem.name().as_ref() == b"cant-bet" => loop {
                if let Event::End(end) = self.next(xml, buf).await?
                    && end.name().as_ref() == b"cant-bet"
                {
                    break;
                }
            },
            _ => bail!("expected bet"),
        }

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
            Event::Empty(elem) if elem.name().as_ref() == b"sdt-target" => 'sdt_target: {
                let Some(target_text) = get_text_attr(&elem)? else {
                    break 'sdt_target;
                };

                // TODO redundant allocation
                let target_text = target_text.into_owned();

                if !self.write_besifrei_note(xml, buf, writer).await? {
                    self.write_text(writer, &target_text).await?;
                }
            }
            e => bail!("expected sdt-target, found: {e:?}"),
        }
        loop {
            let Event::End(end) = self.next(xml, buf).await? else {
                continue;
            };
            if end.name().as_ref() == b"scrdfftar" {
                return Ok(());
            }
        }
    }

    async fn write_besifrei_note(
        &mut self,
        xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
        buf: &mut Vec<u8>,
        writer: &mut BufWriter<File>,
    ) -> eyre::Result<bool> {
        let note = match self.next(xml, buf).await? {
            // complicated note, the ones I've seen don't seem to be important
            Event::Start(elem) if elem.name().as_ref() == b"sdt-note" => return Ok(false),
            Event::Empty(elem) if elem.name().as_ref() == b"sdt-note" => elem,
            unexpected => bail!("unexpected {unexpected:?}"),
        };

        let Some(note_text) = get_text_attr(&note)? else {
            bail!("expected note text");
        };

        // TODO this is sad
        if note_text == "בספרי ספרד ואשכנז וי״ו קטיעא".as_bytes() {
            return Ok(false);
        }

        if let Some(note_text) = note_text.strip_prefix("בספרי ספרד ואשכנז ".as_bytes())
        {
            self.write_text(writer, note_text).await?;
            return Ok(true);
        }

        if let Some(note_text) = note_text.strip_prefix("בספרי ספרד ורוב ספרי אשכנז ".as_bytes())
        {
            self.write_text(writer, note_text).await?;
            return Ok(true);
        }

        Ok(false)
    }

    async fn parse_slh_word(
        &mut self,
        xml: &mut quick_xml::Reader<impl AsyncBufRead + Unpin>,
        buf: &mut Vec<u8>,
        writer: &mut BufWriter<File>,
    ) -> eyre::Result<()> {
        loop {
            let event = self.next(xml, buf).await?;

            match self.sof_pasuq(&event, writer).await {
                Some(Ok(())) => continue,
                Some(Err(err)) => return Err(err),
                None => (),
            }

            match event {
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
