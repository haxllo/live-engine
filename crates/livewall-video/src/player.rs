use std::path::{Path, PathBuf};
use std::time::Duration;

use livewall_pack::{LoopMode, WallpaperKind, WallpaperManifest};
use thiserror::Error;

use crate::PlaybackClock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoDescriptor {
    pub wallpaper_id: String,
    pub title: String,
    pub video_path: PathBuf,
    pub loop_mode: LoopMode,
}

#[derive(Debug)]
pub struct VideoPlayer {
    pub descriptor: VideoDescriptor,
    pub clock: PlaybackClock,
    #[cfg(windows)]
    pub(crate) reader: windows::Win32::Media::MediaFoundation::IMFSourceReader,
}

#[derive(Debug, Error)]
pub enum VideoPlayerError {
    #[error("video playback is only available on Windows")]
    UnsupportedPlatform,
    #[error("wallpaper `{0}` is not a video package")]
    WrongWallpaperKind(String),
    #[error("video file `{0}` is missing")]
    MissingVideo(PathBuf),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{context} failed: {message}")]
    Platform {
        context: &'static str,
        message: String,
    },
}

pub fn load_video_descriptor(
    manifest: &WallpaperManifest,
    install_dir: &Path,
) -> Result<VideoDescriptor, VideoPlayerError> {
    let WallpaperKind::Video { entry, loop_mode } = &manifest.kind else {
        return Err(VideoPlayerError::WrongWallpaperKind(manifest.id.clone()));
    };

    let video_path = install_dir.join(entry);
    if !video_path.is_file() {
        return Err(VideoPlayerError::MissingVideo(entry.clone()));
    }

    Ok(VideoDescriptor {
        wallpaper_id: manifest.id.clone(),
        title: manifest.title.clone(),
        video_path,
        loop_mode: *loop_mode,
    })
}

#[cfg(windows)]
impl VideoPlayer {
    pub fn open(
        descriptor: VideoDescriptor,
        duration: Duration,
        fps: f32,
    ) -> Result<Self, VideoPlayerError> {
        use std::os::windows::ffi::OsStrExt;

        use windows::Win32::Media::MediaFoundation::{
            IMFAttributes, MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING,
            MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, MF_VERSION, MFCreateAttributes,
            MFCreateSourceReaderFromURL, MFStartup,
        };

        unsafe {
            MFStartup(MF_VERSION, 0).map_err(|error| VideoPlayerError::Platform {
                context: "MFStartup",
                message: error.to_string(),
            })?;
        }

        let mut attributes = None::<IMFAttributes>;
        unsafe {
            MFCreateAttributes(&mut attributes, 2).map_err(|error| VideoPlayerError::Platform {
                context: "MFCreateAttributes",
                message: error.to_string(),
            })?;
        }
        let attributes = attributes.ok_or_else(|| VideoPlayerError::Platform {
            context: "MFCreateAttributes",
            message: "Media Foundation attributes were not returned".to_string(),
        })?;

        unsafe {
            attributes
                .SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)
                .map_err(|error| VideoPlayerError::Platform {
                    context: "IMFAttributes::SetUINT32",
                    message: error.to_string(),
                })?;
            attributes
                .SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
                .map_err(|error| VideoPlayerError::Platform {
                    context: "IMFAttributes::SetUINT32",
                    message: error.to_string(),
                })?;
        }

        let path_wide: Vec<u16> = descriptor
            .video_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let reader = unsafe {
            MFCreateSourceReaderFromURL(
                windows::core::PCWSTR(path_wide.as_ptr()),
                Some(&attributes),
            )
            .map_err(|error| VideoPlayerError::Platform {
                context: "MFCreateSourceReaderFromURL",
                message: error.to_string(),
            })?
        };

        Ok(Self {
            clock: PlaybackClock::new(duration, descriptor.loop_mode, fps),
            descriptor,
            reader,
        })
    }
}

#[cfg(not(windows))]
impl VideoPlayer {
    pub fn open(
        descriptor: VideoDescriptor,
        duration: Duration,
        fps: f32,
    ) -> Result<Self, VideoPlayerError> {
        let _ = (descriptor, duration, fps);
        Err(VideoPlayerError::UnsupportedPlatform)
    }
}
