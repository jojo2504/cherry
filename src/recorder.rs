use std::{fmt::Debug, mem::MaybeUninit, thread};
use tokio::sync::mpsc::Sender;

use ashpd::desktop::{
    PersistMode,
    screencast::{CursorMode, Screencast, SourceType},
};
use pipewire::{
    context::Context,
    main_loop::MainLoop,
    properties::Properties,
    spa::{
        param::ParamType,
        pod::{Pod, builder},
        sys::{
            SPA_CHOICE_Enum, SPA_CHOICE_Range, SPA_FORMAT_VIDEO_format, SPA_FORMAT_VIDEO_framerate,
            SPA_FORMAT_VIDEO_size, SPA_FORMAT_mediaSubtype, SPA_FORMAT_mediaType,
            SPA_MEDIA_SUBTYPE_raw, SPA_MEDIA_TYPE_video, SPA_PARAM_EnumFormat,
            SPA_TYPE_OBJECT_Format, SPA_VIDEO_FORMAT_BGRx, SPA_VIDEO_FORMAT_I420,
            SPA_VIDEO_FORMAT_RGB, SPA_VIDEO_FORMAT_RGBA, SPA_VIDEO_FORMAT_RGBx,
            SPA_VIDEO_FORMAT_YUY2, spa_format_parse, spa_format_video_raw_parse, spa_pod_frame,
            spa_video_info,
        },
        utils::{Direction, Fraction, Id, Rectangle},
    },
    stream::{Stream, StreamFlags},
};

#[derive(Clone, Copy)]
struct Data {
    format: Option<spa_video_info>,
}

impl Debug for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(fmt) = self.format {
            write!(
                f,
                "media_type={} subtype={}",
                fmt.media_type, fmt.media_subtype
            )
        } else {
            write!(f, "no format")
        }
    }
}

pub struct RecorderWindows {}

impl RecorderWindows {
    fn new() -> Self {
        RecorderWindows {}
    }

    fn start_recording(&self) {
        // Windows-specific screen recording implementation
    }
}

pub struct RawFrame {
    pub data: Vec<u8>,
    pub format: u32,
    pub width: u32,
    pub height: u32,
}

impl RawFrame {
    fn new(data: Vec<u8>, format: u32, width: u32, height: u32) -> Self {
        Self {
            data,
            format,
            width,
            height,
        }
    }
}

pub struct RecorderLinux<'a> {
    proxy: Screencast<'a>,
}

impl<'a> RecorderLinux<'a> {
    pub async fn new() -> ashpd::Result<RecorderLinux<'a>> {
        Ok(Self {
            proxy: Screencast::new().await?,
        })
    }

    pub async fn start_recording(&self, sender: Sender<RawFrame>) -> Result<(), anyhow::Error> {
        println!("Starting recording...");

        // --- Portal negotiation (async, runs on tokio) ---
        let session = self.proxy.create_session().await?;
        println!("Session created");

        self.proxy
            .select_sources(
                &session,
                CursorMode::Embedded,
                SourceType::Monitor | SourceType::Window,
                true,
                None,
                PersistMode::DoNot,
            )
            .await?;
        println!("Sources selected");

        let response = self.proxy.start(&session, None).await?.response()?;
        let node_id = response.streams()[0].pipe_wire_node_id();
        println!("Stream obtained: node_id = {}", node_id);

        // --- Hand everything PipeWire-related to a plain OS thread ---
        // MainLoop / Context / Stream / Listener are all !Send, so they must
        // live and die on this one thread.
        thread::Builder::new()
            .name("pipewire-capture".into())
            .spawn(move || {
                if let Err(e) = run_pipewire_loop(node_id, sender) {
                    eprintln!("[pipewire thread] error: {e}");
                }
            })?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Everything PipeWire lives here — one dedicated OS thread, never touched by tokio.
// ---------------------------------------------------------------------------
fn run_pipewire_loop(node_id: u32, sender: Sender<RawFrame>) -> Result<(), anyhow::Error> {
    // Build the format-negotiation POD before touching the mainloop so the
    // temporary Vec borrow doesn't overlap with anything else.
    let pod_bytes = build_format_pod()?;
    println!("POD built, buffer size: {}", pod_bytes.len());

    let mut props = Properties::new();
    props.insert("media.type", "Video");
    props.insert("media.category", "Capture");
    props.insert("media.role", "Screen");

    let mainloop = MainLoop::new(None)?;
    let context = Context::new(&mainloop)?;
    let core = context.connect(None)?;
    let pw_stream = Stream::new(&core, "screen_capture", props)?;

    let user_data = Data { format: None };

    // _listener must stay alive for the duration of mainloop.run()
    let _listener = pw_stream
        .add_local_listener_with_user_data(user_data)
        // ── state changes ────────────────────────────────────────────────
        .state_changed(|_stream, _data, old, new| {
            println!("Stream state changed: {:?} -> {:?}", old, new);
        })
        // ── format negotiation ───────────────────────────────────────────
        .param_changed(|_stream, data: &mut Data, id, param| {
            let Some(param) = param else { return };
            if id != ParamType::Format.as_raw() {
                return;
            }

            unsafe {
                let mut info = spa_video_info {
                    media_type: 0,
                    media_subtype: 0,
                    info: std::mem::zeroed(),
                };

                if spa_format_parse(
                    param.as_raw_ptr(),
                    &mut info.media_type as *mut _,
                    &mut info.media_subtype as *mut _,
                ) < 0
                {
                    return;
                }

                if info.media_type != SPA_MEDIA_TYPE_video
                    || info.media_subtype != SPA_MEDIA_SUBTYPE_raw
                {
                    return;
                }

                if spa_format_video_raw_parse(param.as_raw_ptr(), &mut info.info.raw as *mut _) < 0
                {
                    return;
                }

                data.format = Some(info);

                println!("got video format:");
                println!(
                    "  format: {} ({})",
                    info.info.raw.format, info.info.raw.format
                );
                println!(
                    "  size: {}x{}",
                    info.info.raw.size.width, info.info.raw.size.height
                );
                println!(
                    "  framerate: {}/{}",
                    info.info.raw.framerate.num, info.info.raw.framerate.denom
                );
            }
        })
        // ── frame delivery ───────────────────────────────────────────────
        .process(move |stream, data_fmt| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };

            let datas = buffer.datas_mut();
            let Some(spa_data) = datas.first_mut() else {
                return;
            };

            // Inspect raw spa_data fields directly.
            // We intentionally avoid spa_data.data() because it returns a
            // slice of `maxsize` bytes which can overrun the mmap'd region
            // and trigger a SIGSEGV inside AVX-512 memcpy.
            let raw = spa_data.as_raw();

            if raw.chunk.is_null() {
                eprintln!("[pipewire] chunk ptr is null, skipping frame");
                return;
            }

            let (offset, size, _stride) = unsafe {
                (
                    (*raw.chunk).offset as usize,
                    (*raw.chunk).size as usize,
                    (*raw.chunk).stride as usize,
                )
            };

            if size == 0 {
                return;
            }

            // Copy only the valid bytes using the fd + mmap approach
            // so we never touch memory outside [offset, offset+size).
            let frame_bytes = copy_frame_bytes(raw, offset, size);
            let Some(frame_bytes) = frame_bytes else {
                eprintln!("[pipewire] could not read frame bytes, skipping");
                return;
            };

            println!("Frame captured: {} bytes", frame_bytes.len());

            if let Some(fmt) = data_fmt.format {
                let raw_frame = unsafe {
                    RawFrame::new(
                        frame_bytes,
                        fmt.info.raw.format,
                        fmt.info.raw.size.width,
                        fmt.info.raw.size.height,
                    )
                };
                if sender.try_send(raw_frame).is_err() {
                    eprintln!("[pipewire] encoder channel full, dropping frame");
                }
            }
        })
        .register()?;

    let pod_ref = Pod::from_bytes(&pod_bytes).expect("invalid POD bytes");
    let mut params = [pod_ref];

    pw_stream.connect(
        Direction::Input,
        Some(node_id),
        // No MAP_BUFFERS — we mmap the fd ourselves with exact bounds
        // to avoid AVX-512 overread past the end of the mapped region.
        StreamFlags::AUTOCONNECT | StreamFlags::RT_PROCESS,
        &mut params,
    )?;

    println!("Stream connected, running mainloop...");
    mainloop.run();

    Ok(())
}

// ---------------------------------------------------------------------------
// Copy exactly `size` bytes at `offset` from the spa_data buffer.
//
// With MAP_BUFFERS removed, PipeWire gives us a MemFd.  We mmap only the
// exact range [offset, offset+size) so AVX-512 wide loads can never reach
// an unmapped guard page.
//
// Falls back to the data pointer (MemPtr type) when there is no fd.
// ---------------------------------------------------------------------------
fn copy_frame_bytes(
    raw: &pipewire::spa::sys::spa_data,
    offset: usize,
    size: usize,
) -> Option<Vec<u8>> {
    use pipewire::spa::sys::{SPA_DATA_MemFd, SPA_DATA_MemPtr};

    match raw.type_ {
        // ── MemFd: mmap the fd ourselves with exact bounds ───────────────
        t if t == SPA_DATA_MemFd => {
            let fd = raw.fd as std::os::unix::io::RawFd;
            if fd < 0 {
                return None;
            }

            // mmap only the pages that cover [offset, offset+size).
            // We round down to the page boundary for the mmap call and
            // then adjust the slice start inside the mapping.
            let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };
            let map_offset = (offset / page_size) * page_size;
            let map_size = (offset - map_offset) + size;

            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    map_size,
                    libc::PROT_READ,
                    libc::MAP_SHARED,
                    fd,
                    map_offset as libc::off_t,
                )
            };

            if ptr == libc::MAP_FAILED {
                eprintln!(
                    "[pipewire] mmap failed: {}",
                    std::io::Error::last_os_error()
                );
                return None;
            }

            let inner_offset = offset - map_offset;
            let bytes = unsafe {
                let src = (ptr as *const u8).add(inner_offset);
                std::slice::from_raw_parts(src, size).to_vec()
            };

            unsafe { libc::munmap(ptr, map_size) };

            Some(bytes)
        }

        // ── MemPtr: data pointer is already mapped for us ────────────────
        t if t == SPA_DATA_MemPtr => {
            if raw.data.is_null() {
                return None;
            }
            let bytes = unsafe {
                let src = (raw.data as *const u8).add(offset);
                std::slice::from_raw_parts(src, size).to_vec()
            };
            Some(bytes)
        }

        other => {
            eprintln!("[pipewire] unsupported spa_data type: {other}");
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Build the SPA format-negotiation POD.
// ---------------------------------------------------------------------------
fn build_format_pod() -> Result<Vec<u8>, anyhow::Error> {
    let mut buf = Vec::with_capacity(4096);
    let mut pod_builder = builder::Builder::new(&mut buf);
    let mut frame: MaybeUninit<spa_pod_frame> = MaybeUninit::uninit();
    let mut choice_frame: MaybeUninit<spa_pod_frame> = MaybeUninit::uninit();

    unsafe {
        pod_builder.push_object(&mut frame, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat)?;

        pod_builder.add_prop(SPA_FORMAT_mediaType, 0)?;
        pod_builder.add_id(Id(SPA_MEDIA_TYPE_video))?;

        pod_builder.add_prop(SPA_FORMAT_mediaSubtype, 0)?;
        pod_builder.add_id(Id(SPA_MEDIA_SUBTYPE_raw))?;

        // Pixel formats — compositor picks the best match from this list.
        // BGRx is listed first as the preferred (default) choice.
        pod_builder.add_prop(SPA_FORMAT_VIDEO_format, 0)?;
        pod_builder.push_choice(&mut choice_frame, SPA_CHOICE_Enum, 0)?;
        pod_builder.add_id(Id(SPA_VIDEO_FORMAT_BGRx))?; // default
        pod_builder.add_id(Id(SPA_VIDEO_FORMAT_I420))?;
        pod_builder.add_id(Id(SPA_VIDEO_FORMAT_YUY2))?;
        pod_builder.add_id(Id(SPA_VIDEO_FORMAT_RGBx))?;
        pod_builder.add_id(Id(SPA_VIDEO_FORMAT_RGBA))?;
        pod_builder.add_id(Id(SPA_VIDEO_FORMAT_RGB))?;
        pod_builder.pop(choice_frame.assume_init_mut());

        // Resolution range
        pod_builder.add_prop(SPA_FORMAT_VIDEO_size, 0)?;
        pod_builder.push_choice(&mut choice_frame, SPA_CHOICE_Range, 0)?;
        pod_builder.add_rectangle(Rectangle {
            width: 1920,
            height: 1080,
        })?; // default
        pod_builder.add_rectangle(Rectangle {
            width: 1,
            height: 1,
        })?; // min
        pod_builder.add_rectangle(Rectangle {
            width: 7680,
            height: 4320,
        })?; // max
        pod_builder.pop(choice_frame.assume_init_mut());

        // Framerate range
        pod_builder.add_prop(SPA_FORMAT_VIDEO_framerate, 0)?;
        pod_builder.push_choice(&mut choice_frame, SPA_CHOICE_Range, 0)?;
        pod_builder.add_fraction(Fraction { num: 30, denom: 1 })?; // default
        pod_builder.add_fraction(Fraction { num: 0, denom: 1 })?; // min
        pod_builder.add_fraction(Fraction {
            num: 1000,
            denom: 1,
        })?; // max
        pod_builder.pop(choice_frame.assume_init_mut());

        pod_builder.pop(frame.assume_init_mut());
    }

    Ok(buf)
}
