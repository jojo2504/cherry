use std::{fmt::Debug, mem::MaybeUninit, os::raw};
use tokio::{runtime::Handle, sync::mpsc::Sender};

use ashpd::desktop::{
    screencast::{CursorMode, Screencast, SourceType},
    PersistMode,
};
use pipewire::{
    context::Context,
    main_loop::MainLoop,
    properties::Properties,
    spa::{
        param::ParamType, pod::{Pod, builder}, sys::{
            SPA_CHOICE_Enum, SPA_CHOICE_Range, SPA_FORMAT_VIDEO_format, SPA_FORMAT_VIDEO_framerate, SPA_FORMAT_VIDEO_size, SPA_FORMAT_mediaSubtype, SPA_FORMAT_mediaType, SPA_MEDIA_SUBTYPE_raw, SPA_MEDIA_TYPE_video, SPA_PARAM_EnumFormat, SPA_TYPE_OBJECT_Format, SPA_VIDEO_FORMAT_BGRx, SPA_VIDEO_FORMAT_I420, SPA_VIDEO_FORMAT_RGB, SPA_VIDEO_FORMAT_RGBA, SPA_VIDEO_FORMAT_RGBx, SPA_VIDEO_FORMAT_YUY2, spa_format_parse, spa_format_video_raw_parse, spa_pod_frame, spa_video_info
        }, utils::{Direction, Fraction, Id, Rectangle}
    },
    stream::{Stream, StreamFlags},
};

#[cfg(target_os = "windows")]
pub type ScreenRecorder<'a> = RecorderWindows<'a>;

#[cfg(target_os = "linux")]
pub type ScreenRecorder<'a> = RecorderLinux<'a>; 

struct Data {
    format: Option<spa_video_info>
}

impl Debug for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(_format) = self.format {
            write!(f, "{:?} {:?}", _format.media_type, _format.media_subtype)
        }
        else {
            write!(f, "")
        }
    }
}

pub struct RecorderWindows {
    
}

impl RecorderWindows {
    fn new() -> Self {
        RecorderWindows {

        }
    }

    fn start_recording(&self) {
        // Windows-specific screen recording implementation
    }
}

pub struct RawFrame {
    pub data: Vec<u8>,
    pub format: u32,
    pub width: u32,
    pub height: u32
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
    proxy: Screencast<'a>
}

impl<'a> RecorderLinux<'a> {
    pub async fn new() -> ashpd::Result<RecorderLinux<'a>> {
        Ok(Self {
            proxy: Screencast::new().await?
        })
    }

    pub async fn start_recording(&self, sender: Sender<RawFrame>) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        // Linux-specific screen recording implementation
        println!("Starting recording...");

        // use ashpd to get a pipewire valid node
        let session = self.proxy.create_session().await?;
        println!("Session created");

        self.proxy.select_sources(
            &session,
            CursorMode::Embedded,
            SourceType::Monitor | SourceType::Window,
            true,
            None,
            PersistMode::DoNot,
        ).await?;
        println!("Sources selected");

        let response = self.proxy.start(&session, None).await?.response()?;
        let stream = &response.streams()[0];
        println!("Stream obtained: node_id = {}", stream.pipe_wire_node_id());

        // pipewire handling
        let mut buffer: Vec<u8> = vec![];
        let mut pod_builder = builder::Builder::new(&mut buffer);
        
        let mut props = Properties::new();
        props.insert("media.type", "Video");
        props.insert("media.category", "Capture");
        props.insert("media.role", "Screen");
        println!("Properties created");

        let mainloop = MainLoop::new(None)?;
        let mainloop_closure = mainloop.clone();

        let context = Context::new(&mainloop)?;
        let core = context.connect(None)?;
        let pw_stream = Stream::new(&core, "screen_capture", props)?;

        let data: Data = Data {
            format: None
        };

        let _listener = pw_stream.add_local_listener_with_user_data(data)
            .state_changed(|stream, data, old, new| {
                println!("Stream state changed: {:?} -> {:?}", old, new);
            })
            .param_changed(move |stream, data, id, param| {
                if param.is_none() || id != ParamType::Format.as_raw() {
                    return;
                }
                let param = param.unwrap();
        
            unsafe {
                let mut format = spa_video_info {
                    media_type: 0,
                    media_subtype: 0,
                    info: std::mem::zeroed(),
                };
                
                // Parse the format
                if spa_format_parse(
                    param.as_raw_ptr(),
                    &mut format.media_type as *mut _,
                    &mut format.media_subtype as *mut _,
                ) < 0 {
                    return;
                }
                
                // Check if it's video/raw
                if format.media_type != SPA_MEDIA_TYPE_video ||
                format.media_subtype != SPA_MEDIA_SUBTYPE_raw {
                    return;
                }
                
                // Parse video format details
                if spa_format_video_raw_parse(param.as_raw_ptr(), &mut format.info.raw as *mut _) < 0 {
                    return;
                }
                
                // Store format in data
                data.format = Some(format);
                
                // Print format info
                println!("got video format:");
                println!("  format: {} ({})", 
                    format.info.raw.format,
                    format.info.raw.format // You'd need spa_debug_type_find_name for the string
                );
                println!("  size: {}x{}", 
                    format.info.raw.size.width,
                    format.info.raw.size.height
                );
                println!("  framerate: {}/{}",
                    format.info.raw.framerate.num,
                    format.info.raw.framerate.denom
                );
            }
            })
            .process(move |stream, data_format| {
                // println!("{:?}", stream.properties());
                if let Some(mut buffer) = stream.dequeue_buffer() {
                    // Process buffer data
                    // println!("Processing buffer frame");
                    let datas = buffer.datas_mut();
                    if let Some(data) = datas.first_mut() {
                        if let Some(internal_data) = data.data() {
                            let expected = 2560 * 1440 * 4;
                            println!("Frame size: {} (expected: {})", internal_data.len(), expected);

                            let format = data_format.format.unwrap();
                            unsafe {
                                let raw_frame = RawFrame::new(
                                    internal_data.to_vec(),
                                    format.info.raw.format,
                                    format.info.raw.size.width, 
                                    format.info.raw.size.height,
                                );
                                sender.try_send(raw_frame).unwrap();
                            }                 
                        }
                        // println!("Got frame data: {:?}", data.data());
                        mainloop_closure.quit();
                    }
                }
            })
            .register()?;
        
        println!("Building POD...");
        let mut frame: MaybeUninit<spa_pod_frame> = MaybeUninit::uninit();
        let mut choice_frame: MaybeUninit<spa_pod_frame> = MaybeUninit::uninit();
        unsafe {
            // Push object
            pod_builder.push_object(&mut frame, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat)?;
            
            // Media type
            pod_builder.add_prop(SPA_FORMAT_mediaType, 0)?;
            pod_builder.add_id(Id(SPA_MEDIA_TYPE_video))?;
            
            // Media subtype
            pod_builder.add_prop(SPA_FORMAT_mediaSubtype, 0)?;
            pod_builder.add_id(Id(SPA_MEDIA_SUBTYPE_raw))?;
            
            // Video format (with choice enum)
            pod_builder.add_prop(SPA_FORMAT_VIDEO_format, 0)?;
            pod_builder.push_choice(&mut choice_frame, SPA_CHOICE_Enum, 0)?;
            pod_builder.add_id(Id(SPA_VIDEO_FORMAT_RGB))?; // default
            pod_builder.add_id(Id(SPA_VIDEO_FORMAT_RGB))?;
            pod_builder.add_id(Id(SPA_VIDEO_FORMAT_RGBA))?;
            pod_builder.add_id(Id(SPA_VIDEO_FORMAT_RGBx))?;
            pod_builder.add_id(Id(SPA_VIDEO_FORMAT_BGRx))?;
            pod_builder.add_id(Id(SPA_VIDEO_FORMAT_YUY2))?;
            pod_builder.add_id(Id(SPA_VIDEO_FORMAT_I420))?;
            pod_builder.pop(choice_frame.assume_init_mut());
            
            // Video size (with choice range)
            pod_builder.add_prop(SPA_FORMAT_VIDEO_size, 0)?;
            pod_builder.push_choice(&mut choice_frame, SPA_CHOICE_Range, 0)?;
            pod_builder.add_rectangle(Rectangle { width: 320, height: 240 })?; // default
            pod_builder.add_rectangle(Rectangle { width: 1, height: 1 })?; // min
            pod_builder.add_rectangle(Rectangle { width: 4096, height: 4096 })?; // max
            pod_builder.pop(choice_frame.assume_init_mut());
            
            // Video framerate (with choice range)
            pod_builder.add_prop(SPA_FORMAT_VIDEO_framerate, 0)?;
            pod_builder.push_choice(&mut choice_frame, SPA_CHOICE_Range, 0)?;
            pod_builder.add_fraction(Fraction { num: 30, denom: 1 })?; // default
            pod_builder.add_fraction(Fraction { num: 0, denom: 1 })?; // min
            pod_builder.add_fraction(Fraction { num: 1000, denom: 1 })?; // max
            pod_builder.pop(choice_frame.assume_init_mut());
            
            // Pop object
            pod_builder.pop(frame.assume_init_mut());
        }
        println!("POD built, buffer size: {}", buffer.len());

        let pod = Pod::from_bytes(&buffer).unwrap();
        let mut params = [pod];

        pw_stream.connect(
            Direction::Input,
            Some(stream.pipe_wire_node_id()), 
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS, 
            &mut params)?;   
        
        println!("Stream connected, running mainloop...");
        mainloop.run();

        Ok(())
    }
}
