use std::{mem::MaybeUninit, process::exit};

use ashpd::desktop::{
    screencast::{CursorMode, Screencast, SourceType},
    PersistMode,
};
use pipewire::spa::pod;
use pipewire::{
    context::Context,
    core::Core,
    keys::NODE_AUTOCONNECT,
    main_loop::{self, MainLoop},
    properties::Properties,
    spa::{
        __builder_add__,
        buffer::DataType,
        param::{
            format::{FormatProperties, MediaSubtype, MediaType},
            video::{VideoFormat, VideoInfoRaw},
            ParamType,
        },
        pod::{builder, Pod},
        sys::{
            spa_format_parse, spa_param_type, spa_pod_builder, spa_pod_frame, spa_video_info,
            SPA_FORMAT_VIDEO_format, SPA_FORMAT_VIDEO_framerate, SPA_FORMAT_VIDEO_size,
            SPA_FORMAT_mediaSubtype, SPA_FORMAT_mediaType, SPA_MEDIA_SUBTYPE_raw,
            SPA_MEDIA_TYPE_video, SPA_PARAM_EnumFormat, SPA_TYPE_OBJECT_Format,
            SPA_VIDEO_FORMAT_BGRx, SPA_VIDEO_FORMAT_RGBx, SPA_VIDEO_FORMAT_I420,
            SPA_VIDEO_FORMAT_RGB, SPA_VIDEO_FORMAT_RGBA, SPA_VIDEO_FORMAT_YUY2,
        },
        utils::{ChoiceFlags, Direction, Fraction, Id, Rectangle},
    },
    stream::{Stream, StreamFlags},
    sys::{pw_init, PW_KEY_MEDIA_CATEGORY, PW_KEY_MEDIA_ROLE, PW_KEY_MEDIA_TYPE},
};
use pipewire_sys::pw_stream_flags_PW_STREAM_FLAG_MAP_BUFFERS;
