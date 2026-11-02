//! `sugoi-popup`: a bar widget's details, under the pointer, until the next click or key
//! (`doc/spec/popup.md`).

use std::io::Read;
use std::process::ExitCode;
use std::thread::sleep;
use std::time::{Duration, Instant};

use x11rb::connection::{Connection, RequestConnection as _};
use x11rb::protocol::Event;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{COPY_DEPTH_FROM_PARENT, CURRENT_TIME, NONE};

type Error = Box<dyn std::error::Error>;

/// [O1] The style, in pixels at 96 DPI.
const FONT: &str = "monospace 10";
const FOREGROUND: (f64, f64, f64) = (0xaa as f64 / 255.0, 0xaa as f64 / 255.0, 0xaa as f64 / 255.0);
const BACKGROUND: (f64, f64, f64) = (0x22 as f64 / 255.0, 0x22 as f64 / 255.0, 0x22 as f64 / 255.0);
const BORDER_COLOR: (f64, f64, f64) = (0xaa as f64 / 255.0, 0.0, 0x7b as f64 / 255.0);
const MARGIN: f64 = 12.0;
const BORDER: f64 = 1.0;
/// [O2]
const GAP: f64 = 5.0;
/// [O3]
const GRAB_WAIT: Duration = Duration::from_secs(1);

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("sugoi-popup: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, Error> {
    let mut markup = String::new();
    std::io::stdin().read_to_string(&mut markup)?;
    let markup = markup.trim_end();
    if markup.is_empty() {
        return Ok(ExitCode::SUCCESS);
    }
    let (conn, screen) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen];
    let resources = x11rb::resource_manager::new_from_default(&conn)?;
    let dpi = resources.get_value::<f64>("Xft.dpi", "").ok().flatten().unwrap_or(96.0);
    let scale = dpi / 96.0;
    // [O1] The pixels go out as cairo's RGB24: 32 bits a pixel, least significant first.
    let rgb24 = conn.setup().pixmap_formats.iter().any(|f| f.depth == 24 && f.bits_per_pixel == 32);
    if screen.root_depth != 24 || !rgb24 || conn.setup().image_byte_order != ImageOrder::LSB_FIRST {
        eprintln!("sugoi-popup: needs a 24-bit TrueColor screen");
        return Ok(ExitCode::FAILURE);
    }

    // [O2] Under the pointer, below the work area's top, within the screen.
    let pointer = conn.query_pointer(screen.root)?.reply()?;
    let top = work_area_top(&conn, screen.root)?;
    let y = top + (GAP * scale).round() as i32;
    let screen_width = i32::from(screen.width_in_pixels);
    let room = (screen_width, (i32::from(screen.height_in_pixels) - y).max(1));
    let image = render(markup, dpi, scale, room)?;
    let (width, height) = (image.width as i32, image.height as i32);
    let x = (i32::from(pointer.root_x) - width / 2).clamp(0, (screen_width - width).max(0));

    let window = conn.generate_id()?;
    let aux = CreateWindowAux::new()
        .override_redirect(1)
        .background_pixel(0x222222)
        .event_mask(EventMask::EXPOSURE);
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        window,
        screen.root,
        x as i16,
        y as i16,
        width as u16,
        height as u16,
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &aux,
    )?;
    let class = b"sugoi-popup\0sugoi-popup\0";
    conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_CLASS, AtomEnum::STRING, class)?;
    let gc = conn.generate_id()?;
    conn.create_gc(gc, window, &CreateGCAux::new())?;
    if !grab(&conn, screen.root)? {
        eprintln!("sugoi-popup: another client holds the pointer or keyboard");
        return Ok(ExitCode::FAILURE);
    }
    conn.map_window(window)?;
    conn.flush()?;
    loop {
        match conn.wait_for_event()? {
            Event::Expose(e) if e.count == 0 => put(&conn, window, gc, &image, screen.root_depth)?,
            Event::ButtonPress(_) | Event::KeyPress(_) => break,
            _ => {}
        }
    }
    conn.ungrab_pointer(CURRENT_TIME)?;
    conn.ungrab_keyboard(CURRENT_TIME)?;
    conn.destroy_window(window)?;
    conn.flush()?;
    Ok(ExitCode::SUCCESS)
}

/// The popup's pixels, in the server's 24-bit `ZPixmap` order (cairo's RGB24).
struct Image {
    width: u32,
    height: u32,
    stride: usize,
    data: Vec<u8>,
}

/// [O1] [O2] Lays the markup out, or the text when it does not parse, and draws it,
/// within `room`: what does not fit is cut inside the border.
fn render(markup: &str, dpi: f64, scale: f64, room: (i32, i32)) -> Result<Image, Error> {
    let layout = |cr: &cairo::Context| {
        let layout = pangocairo::functions::create_layout(cr);
        pangocairo::functions::context_set_resolution(&layout.context(), dpi);
        layout.context_changed();
        layout.set_font_description(Some(&pango::FontDescription::from_string(FONT)));
        if pango::parse_markup(markup, '\0').is_ok() {
            layout.set_markup(markup);
        } else {
            layout.set_text(markup);
        }
        layout
    };
    let probe = cairo::ImageSurface::create(cairo::Format::Rgb24, 1, 1)?;
    let (_, logical) = layout(&cairo::Context::new(&probe)?).pixel_extents();
    let inset = ((MARGIN + BORDER) * scale).round();
    let width = ((f64::from(logical.width()) + 2.0 * inset) as i32).min(room.0);
    let height = ((f64::from(logical.height()) + 2.0 * inset) as i32).min(room.1);

    let mut surface = cairo::ImageSurface::create(cairo::Format::Rgb24, width, height)?;
    {
        let cr = cairo::Context::new(&surface)?;
        let border = (BORDER * scale).round().max(1.0);
        let (w, h) = (f64::from(width), f64::from(height));
        cr.set_source_rgb(BACKGROUND.0, BACKGROUND.1, BACKGROUND.2);
        cr.paint()?;
        // The text within the margins; the border goes on top, so a cut keeps it.
        cr.save()?;
        cr.rectangle(border, border, w - 2.0 * border, h - 2.0 * border);
        cr.clip();
        cr.set_source_rgb(FOREGROUND.0, FOREGROUND.1, FOREGROUND.2);
        cr.move_to(inset - f64::from(logical.x()), inset - f64::from(logical.y()));
        pangocairo::functions::show_layout(&cr, &layout(&cr));
        cr.restore()?;
        cr.set_source_rgb(BORDER_COLOR.0, BORDER_COLOR.1, BORDER_COLOR.2);
        cr.set_line_width(border);
        cr.rectangle(border / 2.0, border / 2.0, w - border, h - border);
        cr.stroke()?;
    }
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data()?.to_vec();
    Ok(Image { width: width as u32, height: height as u32, stride, data })
}

/// [O2] The work area's top, as sugoi publishes it, or the screen's.
fn work_area_top(conn: &RustConnection, root: Window) -> Result<i32, Error> {
    let atom = conn.intern_atom(false, b"_NET_WORKAREA")?.reply()?.atom;
    let reply = conn.get_property(false, root, atom, AtomEnum::CARDINAL, 0, 4)?.reply()?;
    let y = reply.value32().and_then(|mut v| v.nth(1));
    Ok(y.map_or(0, |y| y as i32))
}

/// [O3] Takes the pointer and the keyboard, retrying while another client holds them
/// (the bar, until the click's button is released).
fn grab(conn: &RustConnection, root: Window) -> Result<bool, Error> {
    let start = Instant::now();
    let (mut pointer, mut keyboard) = (false, false);
    while start.elapsed() < GRAB_WAIT {
        let async_ = GrabMode::ASYNC;
        if !pointer {
            let mask = EventMask::BUTTON_PRESS;
            let reply =
                conn.grab_pointer(false, root, mask, async_, async_, NONE, NONE, CURRENT_TIME)?;
            pointer = reply.reply()?.status == GrabStatus::SUCCESS;
        }
        if !keyboard {
            let reply = conn.grab_keyboard(false, root, CURRENT_TIME, async_, async_)?;
            keyboard = reply.reply()?.status == GrabStatus::SUCCESS;
        }
        if pointer && keyboard {
            return Ok(true);
        }
        sleep(Duration::from_millis(10));
    }
    Ok(false)
}

/// Puts the image in strips within the request size limit.
fn put(
    conn: &RustConnection,
    window: Window,
    gc: Gcontext,
    image: &Image,
    depth: u8,
) -> Result<(), Error> {
    let rows = ((conn.maximum_request_bytes() - 64) / image.stride).max(1);
    for (strip, data) in image.data.chunks(rows * image.stride).enumerate() {
        let height = (data.len() / image.stride) as u16;
        let y = (strip * rows) as i16;
        let width = image.width as u16;
        conn.put_image(ImageFormat::Z_PIXMAP, window, gc, width, height, 0, y, 0, depth, data)?;
    }
    conn.flush()?;
    Ok(())
}
