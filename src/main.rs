use anyhow::{Context, Ok};
use chardetng::EncodingDetector;
use clap::{builder::styling::*, Parser};
use comfy_table::{modifiers::*, presets::*, *};
use dialoguer::{console::Term, theme::ColorfulTheme, Select};
use encoding_rs::Encoding;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::{
  fmt::Display,
  fs::File,
  io::{BufRead, BufReader, BufWriter, Read, Seek, Write},
  num::NonZeroUsize,
  path::PathBuf,
  process::exit,
};

fn clap_v3_styles() -> Styles {
  Styles::styled()
    .header(AnsiColor::Yellow.on_default())
    .usage(AnsiColor::Green.on_default())
    .literal(AnsiColor::Green.on_default())
    .placeholder(AnsiColor::Green.on_default())
}

#[derive(Parser)]
#[command(author, version, about, long_about = None, styles = clap_v3_styles())]
struct Cli {
  files: Vec<PathBuf>,
  #[arg(long, default_value_t = 8 * 1024)]
  buf: usize,
  #[arg(long, default_value_t = 128)]
  preview_buf: usize,
  #[arg(long, default_value = "16384")] // 16 KiB
  limit: NonZeroUsize,
  #[arg(short, long, default_value_t = false)]
  confirm: bool,
}

#[allow(unused)]
#[derive(Debug, Clone, Copy)]
enum Interact {
  Convert,
  Cancel,
  Modify,
  ModifyAll,
}

impl Interact {
  const ITEMS: &'static [Self] = &[
    Interact::Convert,
    Interact::Cancel,
    // Interact::Modify,
    // Interact::ModifyAll,
  ];
}

impl Display for Interact {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{self:?}")
  }
}

#[derive(Debug)]
struct DetectedItem {
  path: PathBuf,
  buf_reader: BufReader<File>,
  total: usize,
  encoding: &'static Encoding,
}

fn main() -> anyhow::Result<()> {
  let cli: Cli = Cli::parse();

  if cli.files.is_empty() {
    println!("Nothing to detect, exiting...");
    exit(0);
  }

  let mut table = Table::new();
  table
    .load_preset(UTF8_FULL)
    .apply_modifier(UTF8_ROUND_CORNERS)
    .apply_modifier(UTF8_SOLID_INNER_BORDERS)
    .set_header(vec![
      "File",
      "Encoding",
      &format!("Preview (first {} bytes)", cli.preview_buf),
    ])
    .set_content_arrangement(ContentArrangement::Dynamic)
    .set_constraints([
      ColumnConstraint::UpperBoundary(Width::Percentage(40)),
      ColumnConstraint::Absolute(Width::Fixed(10)),
      ColumnConstraint::UpperBoundary(Width::Percentage(40)),
    ]);

  let term = Term::buffered_stderr();
  let theme = ColorfulTheme::default();

  let prog_sty =
    ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
      .unwrap()
      .progress_chars("##-");

  let file_num = cli.files.len() as u64;

  let detecting_prog = ProgressBar::new(file_num);
  detecting_prog.set_draw_target(ProgressDrawTarget::term(term.clone(), 20));
  detecting_prog.set_style(prog_sty.clone());

  let mut buf = vec![0; cli.buf];
  let mut preview_buf = vec![0; cli.preview_buf];
  let mut detected_items = Vec::with_capacity(cli.files.len());

  for path in cli.files.into_iter() {
    let path_str = path.to_string_lossy();
    detecting_prog.set_message(path_str.to_string());

    let mut file =
      File::open(&path).with_context(|| format!("Unable to open file: `{}`", path.display()))?;
    let file_total = file
      .metadata()
      .map(|m| m.len() as usize)
      .ok()
      .or_else(|| {
        file
          .seek(std::io::SeekFrom::End(0))
          .ok()
          .map(|len| len as usize)
      })
      .context("Failed to get file total size")?;
    let mut buf_rdr = BufReader::new(file);
    let det = detect_file(&mut buf_rdr, Some(file_total), &mut buf, Some(cli.limit))?;
    let encoding = det.encoding;
    buf_rdr.rewind().context("Failed to rewind reader")?;

    let preview_len = cli.preview_buf.min(file_total);
    let preview_buf = &mut preview_buf[..preview_len];
    buf_rdr.read_exact(preview_buf).context("Failed to read")?;
    let (preview, _) = encoding.decode_without_bom_handling(preview_buf);

    table.add_row(vec![
      path_str,
      det.encoding.name().into(),
      take_chars(&preview.replace("\r\n", "\n"), preview_len).into(),
    ]);

    detecting_prog.inc(1);

    detected_items.push(DetectedItem {
      buf_reader: buf_rdr,
      encoding: det.encoding,
      total: file_total,
      path,
    });
  }

  detecting_prog.suspend(|| {
    println!("\n{table}");
  });
  detecting_prog.finish_with_message("All files are detected!");

  let interacts = Interact::ITEMS;

  loop {
    let selected = if cli.confirm {
      Interact::Convert
    } else {
      let selected = Select::with_theme(&theme)
        .with_prompt("Do you want to continue?")
        .items(&interacts)
        .default(0)
        .report(false)
        .interact_on(&term)?;
      interacts[selected]
    };

    match selected {
      Interact::Convert => {
        let convert_prog = ProgressBar::new(file_num);
        convert_prog.set_draw_target(ProgressDrawTarget::term(term.clone(), 20));
        convert_prog.set_style(prog_sty);

        for mut item in detected_items {
          convert_prog.set_message(item.path.to_string_lossy().to_string());
          item
            .buf_reader
            .seek(std::io::SeekFrom::Start(0))
            .context("Failed to seek from start 0")?;

          let mut output = String::with_capacity(item.total);

          let mut to_utf8 = encoding_rs_io::DecodeReaderBytesBuilder::default()
            .strip_bom(true)
            .utf8_passthru(true)
            .encoding(Some(item.encoding))
            .build(item.buf_reader);

          to_utf8
            .read_to_string(&mut output)
            .context("Failed to convert UTF-8")?;

          let file = File::create(item.path).context("Failed recreate file")?;
          let mut buf_wrt = BufWriter::new(file);
          buf_wrt
            .write_all(output.as_bytes())
            .context("Failed to write")?;
          convert_prog.inc(1);
        }
        convert_prog.finish_with_message("All files are converted!");
        println!();
        break;
      },
      Interact::Cancel => {
        println!("Cancelled");
        exit(0);
      },
      Interact::Modify => todo!(),
      Interact::ModifyAll => todo!(),
    }
  }
  Ok(())
}

fn detect_file<R: BufRead>(
  buf_rdr: &mut R,
  total: Option<usize>,
  buf: &mut [u8],
  limit: Option<NonZeroUsize>,
) -> anyhow::Result<DetectedEncoding> {
  let limit = limit.map(|l| l.get()).unwrap_or(usize::MAX);

  let mut cur = 0;
  let mut det = EncodingDetector::new();

  if !buf_rdr
    .fill_buf()
    .context("Unable to get inner buffer")?
    .is_empty()
  {
    let len = buf_rdr.read(buf)?;
    cur += len;
    if let Some((encoding, size)) = Encoding::for_bom(&buf[..len]) {
      return Ok(DetectedEncoding {
        encoding,
        size,
        likely_wrong: false,
      });
    }
    det.feed(&buf[..len], total == Some(cur));
  }

  while !buf_rdr
    .fill_buf()
    .context("Unable to get inner buffer")?
    .is_empty()
    && cur < limit
  {
    let len = buf_rdr.read(buf)?;
    cur += len;
    det.feed(&buf[..len], total == Some(cur));
  }

  let (encoding, to_be_true) = det.guess_assess(None, true);

  Ok(DetectedEncoding {
    encoding,
    likely_wrong: !to_be_true,
    size: cur,
  })
}

#[derive(Debug)]
pub struct DetectedEncoding {
  pub encoding: &'static Encoding,
  pub size: usize,
  pub likely_wrong: bool,
}

pub fn take_chars(s: &str, i: usize) -> &str {
  if i > s.len() {
    return s;
  }
  if s.is_char_boundary(i) {
    return &s[..i];
  }
  if s.is_char_boundary(i + 1) {
    return &s[..i + 1];
  }
  if s.is_char_boundary(i + 2) {
    return &s[..i + 2];
  }
  if s.is_char_boundary(i + 3) {
    return &s[..i + 3];
  }
  unsafe { std::hint::unreachable_unchecked() }
}
