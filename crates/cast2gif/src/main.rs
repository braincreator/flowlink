use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "flowlink-cast2gif", about = "Convert asciinema v2 .cast files to GIF")]
struct Args {
    #[arg(short, long)]
    input: PathBuf,

    #[arg(short, long)]
    output: PathBuf,

    #[arg(long, default_value = "15")]
    fps: u32,

    #[arg(long, default_value = "80")]
    cols: u32,

    #[arg(long, default_value = "24")]
    rows: u32,

    #[arg(long, default_value = "#0a0e1a")]
    bg: String,

    #[arg(long, default_value = "#e1e4ed")]
    fg: String,

    #[arg(long, default_value = "1.5")]
    speed: f64,
}

fn parse_hex_color(s: &str) -> Result<[u8; 3]> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        anyhow::bail!("Invalid color format: {}", s);
    }
    let r = u8::from_str_radix(&s[0..2], 16)?;
    let g = u8::from_str_radix(&s[2..4], 16)?;
    let b = u8::from_str_radix(&s[4..6], 16)?;
    Ok([r, g, b])
}

fn main() -> Result<()> {
    let args = Args::parse();
    let opts = flowlink_cast2gif::ConvertOptions {
        cols: args.cols,
        rows: args.rows,
        fps: args.fps,
        speed: args.speed,
        bg: parse_hex_color(&args.bg)?,
        fg: parse_hex_color(&args.fg)?,
    };

    eprintln!("Converting {} -> {} ({}x{}, {}fps, {}x speed)",
        args.input.display(), args.output.display(),
        args.cols, args.rows, args.fps, args.speed);

    let start = std::time::Instant::now();
    let stats = flowlink_cast2gif::convert(&args.input, &args.output, opts)?;
    let elapsed = start.elapsed();

    eprintln!(
        "Done! {} frames, {:.0}ms content, {:.1}KB GIF, took {:.2}s",
        stats.frames, stats.duration_ms, stats.gif_size as f64 / 1024.0, elapsed.as_secs_f64()
    );

    Ok(())
}
