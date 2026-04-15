// ---------------------------------------------------------------------------
// Banner — sparse blue aurora + FIGlet "Quasar" text reveal
// ---------------------------------------------------------------------------

pub(super) fn print_banner() {
    use std::io::{self, IsTerminal, Write};

    let stdout = io::stdout();
    if !stdout.is_terminal() {
        println!("\n  Quasar\n  Build programs that execute at the speed of light\n");
        return;
    }

    // Terminal animation — if writes fail the banner is non-essential, so we
    // just skip it rather than propagating the error to the caller.
    if animate_banner(&stdout).is_err() {
        // Ensure cursor is visible even if the animation failed partway through
        let _ = write!(stdout.lock(), "\x1b[?25h");
        let _ = stdout.lock().flush();
    }
}

fn animate_banner(stdout: &std::io::Stdout) -> std::io::Result<()> {
    use std::{io::Write, thread, time::Duration};

    // Restore cursor if interrupted during animation
    ctrlc::set_handler(move || {
        print!("\x1b[?25h");
        std::process::exit(130);
    })
    .ok(); // ctrlc handler registration is best-effort (may already be set)

    let mut out = stdout.lock();
    write!(out, "\x1b[?25l")?;

    let w: usize = 70;
    let h: usize = 11; // 1 blank + 7 figlet + 1 blank + 1 tagline + 1 byline
    let n_frames: usize = 22;
    let nebula_w: f32 = 30.0; // width of the sweeping nebula band

    // FIGlet "Quasar" — block style, 7 lines tall
    #[rustfmt::skip]
    let figlet: [&str; 7] = [
        " ██████╗ ██╗   ██╗ █████╗ ███████╗ █████╗ ██████╗ ",
        "██╔═══██╗██║   ██║██╔══██╗██╔════╝██╔══██╗██╔══██╗",
        "██║   ██║██║   ██║███████║███████╗███████║██████╔╝",
        "██║▄▄ ██║██║   ██║██╔══██║╚════██║██╔══██║██╔══██╗",
        "╚██████╔╝╚██████╔╝██║  ██║███████║██║  ██║██║  ██║",
        " ╚══▀▀═╝  ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═╝",
        "",
    ];
    let fig: Vec<Vec<char>> = figlet.iter().map(|l| l.chars().collect()).collect();
    let fig_w = fig.iter().map(|l| l.len()).max().unwrap_or(0);
    let fig_off = w.saturating_sub(fig_w) / 2;

    let tagline = "Build programs that execute at the speed of light";
    let tag_chars: Vec<char> = tagline.chars().collect();
    let tag_off = w.saturating_sub(tag_chars.len()) / 2;

    let byline = "by blueshift.gg";
    let by_chars: Vec<char> = byline.chars().collect();
    let by_off = w.saturating_sub(by_chars.len()) / 2;

    // Reserve space
    writeln!(out)?;
    for _ in 0..h {
        writeln!(out)?;
    }
    out.flush()?;

    for frame in 0..n_frames {
        write!(out, "\x1b[{h}A")?;
        let is_final = frame == n_frames - 1;

        // Leading edge sweeps left → right, revealing text in its wake
        let t = frame as f32 / (n_frames - 2).max(1) as f32;
        let edge = -nebula_w + t * (w as f32 + nebula_w * 2.0);

        #[allow(clippy::needless_range_loop)]
        for li in 0..h {
            write!(out, "\x1b[2K  ")?;

            if is_final {
                // ── Final clean frame ──
                match li {
                    1..=7 => {
                        let row = &fig[li - 1];
                        for _ in 0..fig_off {
                            write!(out, " ")?;
                        }
                        for &ch in row.iter() {
                            if ch != ' ' {
                                write!(out, "\x1b[36m{ch}\x1b[0m")?;
                            } else {
                                write!(out, " ")?;
                            }
                        }
                    }
                    9 => {
                        for _ in 0..tag_off {
                            write!(out, " ")?;
                        }
                        write!(out, "\x1b[1m{tagline}\x1b[0m")?;
                    }
                    10 => {
                        for _ in 0..by_off {
                            write!(out, " ")?;
                        }
                        write!(out, "\x1b[90mby \x1b[36mblueshift.gg\x1b[0m")?;
                    }
                    _ => {}
                }
            } else {
                // ── Nebula sweep: reveals text as it passes ──
                for ci in 0..w {
                    let dist = ci as f32 - edge;

                    // Text character at this position
                    let text_ch = match li {
                        1..=7 if ci >= fig_off && ci - fig_off < fig_w => {
                            fig[li - 1].get(ci - fig_off).copied().unwrap_or(' ')
                        }
                        9 if ci >= tag_off && ci - tag_off < tag_chars.len() => {
                            tag_chars[ci - tag_off]
                        }
                        10 if ci >= by_off && ci - by_off < by_chars.len() => by_chars[ci - by_off],
                        _ => ' ',
                    };

                    if dist < -nebula_w {
                        // Behind the nebula: text fully revealed
                        write_text_char(&mut out, text_ch, li, ci, by_off)?;
                    } else if dist < nebula_w {
                        // Inside the nebula band
                        let blend = (dist + nebula_w) / (nebula_w * 2.0);
                        let intensity = 1.0 - (dist.abs() / nebula_w);
                        let d = aurora_density(ci, li, frame) * intensity;

                        if blend < 0.3 && text_ch != ' ' {
                            // Trailing edge: text bleeds through
                            write_text_char(&mut out, text_ch, li, ci, by_off)?;
                        } else {
                            write_nebula_char(&mut out, d)?;
                        }
                    } else {
                        // Ahead of nebula: dark
                        write!(out, " ")?;
                    }
                }
            }
            writeln!(out)?;
        }
        out.flush()?;

        if !is_final {
            thread::sleep(Duration::from_millis(55));
        }
    }

    write!(out, "\x1b[?25h")?;
    writeln!(out)?;
    out.flush()?;
    Ok(())
}

fn write_text_char(
    out: &mut impl std::io::Write,
    ch: char,
    line: usize,
    col: usize,
    by_off: usize,
) -> std::io::Result<()> {
    if ch == ' ' {
        write!(out, " ")?;
    } else {
        match line {
            1..=7 => {
                write!(out, "\x1b[36m{ch}\x1b[0m")?;
            }
            9 => {
                write!(out, "\x1b[1m{ch}\x1b[0m")?;
            }
            10 => {
                if col - by_off < 3 {
                    write!(out, "\x1b[90m{ch}\x1b[0m")?;
                } else {
                    write!(out, "\x1b[36m{ch}\x1b[0m")?;
                }
            }
            _ => {
                write!(out, " ")?;
            }
        };
    }
    Ok(())
}

fn write_nebula_char(out: &mut impl std::io::Write, d: f32) -> std::io::Result<()> {
    if d < 0.10 {
        write!(out, " ")?;
    } else if d < 0.25 {
        write!(out, "\x1b[38;2;15;25;85m░\x1b[0m")?;
    } else if d < 0.42 {
        write!(out, "\x1b[38;2;30;55;145m░\x1b[0m")?;
    } else if d < 0.60 {
        write!(out, "\x1b[38;2;50;95;200m▒\x1b[0m")?;
    } else if d < 0.78 {
        write!(out, "\x1b[38;2;75;140;235m▓\x1b[0m")?;
    } else {
        write!(out, "\x1b[38;2;100;170;255m█\x1b[0m")?;
    }
    Ok(())
}

/// Aurora density — sine waves flowing rightward, tuned for sparse output.
fn aurora_density(col: usize, line: usize, frame: usize) -> f32 {
    let c = col as f32;
    let l = line as f32;
    let f = frame as f32;

    let w1 = ((c - f * 5.0) / 8.0 + l * 0.35).sin();
    let w2 = ((c - f * 3.5) / 5.5 - l * 0.25).sin() * 0.45;
    let w3 = ((c - f * 7.0) / 12.0 + l * 0.15).sin() * 0.3;

    ((w1 + w2 + w3 + 1.5) / 3.5).clamp(0.0, 1.0)
}
