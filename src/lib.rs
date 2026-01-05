use once_cell::sync::Lazy;
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::Mutex;

use tokio::runtime::Runtime;

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};

static RT: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().expect("Failed to create Tokio runtime")
});

static MEDIA_STATE: Lazy<Mutex<MediaInfo>> =
    Lazy::new(|| Mutex::new(MediaInfo::default()));

#[derive(Default)]
struct MediaInfo {
    title: String,
    artist: String,
    position: i64,
    duration: i64,
    is_playing: bool,
    thumbnail: Vec<u8>,
}

async fn get_session() -> Option<GlobalSystemMediaTransportControlsSession> {
    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .ok()?
        .await
        .ok()?;

    manager.GetCurrentSession().ok()
}

#[no_mangle]
pub extern "system" fn refresh() -> i32 {
    RT.block_on(async {
        let session = match get_session().await {
            Some(s) => s,
            None => return,
        };

        let props = match session.TryGetMediaPropertiesAsync() {
            Ok(op) => match op.await {
                Ok(p) => p,
                Err(_) => return,
            },
            Err(_) => return,
        };

        let timeline = session.GetTimelineProperties().ok();
        let playback = session.GetPlaybackInfo().ok();

        let mut state = MEDIA_STATE.lock().unwrap();

        let new_title = props.Title().unwrap_or_default().to_string();
        let new_artist = props.Artist().unwrap_or_default().to_string();

        let song_changed = new_title != state.title;

        state.title = new_title;
        state.artist = new_artist;

        if let Some(t) = timeline {
            state.position = (t.Position().unwrap().Duration / 10_000_000) as i64;
            state.duration =
                ((t.EndTime().unwrap().Duration - t.StartTime().unwrap().Duration) / 10_000_000)
                    as i64;
        }

        if let Some(p) = playback {
            state.is_playing = p.PlaybackStatus().unwrap_or(
                GlobalSystemMediaTransportControlsSessionPlaybackStatus::Closed,
            ) == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;
        }

        if song_changed {
            state.thumbnail.clear();

            if let Ok(thumbnail_ref) = props.Thumbnail() {
                if let Ok(stream) =
                    IRandomAccessStreamReference::OpenReadAsync(&thumbnail_ref).unwrap().await
                {
                    let size = stream.Size().unwrap_or(0) as usize;
                    if size > 0 {
                        let mut buffer = vec![0u8; size];
                        if let Ok(input) = stream.GetInputStreamAt(0) {
                            let reader = DataReader::CreateDataReader(&input).unwrap();
                            if reader.LoadAsync(size as u32).unwrap().await.is_ok() {
                                let _ = reader.ReadBytes(&mut buffer);
                                state.thumbnail = buffer;
                            }
                        }
                    }
                }
            }
        }
    });

    0
}

#[no_mangle]
pub extern "system" fn getTitle() -> *const c_char {
    CString::new(MEDIA_STATE.lock().unwrap().title.clone())
        .unwrap()
        .into_raw()
}

#[no_mangle]
pub extern "system" fn getArtist() -> *const c_char {
    CString::new(MEDIA_STATE.lock().unwrap().artist.clone())
        .unwrap()
        .into_raw()
}

#[no_mangle]
pub extern "system" fn getPosition() -> i64 {
    MEDIA_STATE.lock().unwrap().position
}

#[no_mangle]
pub extern "system" fn getDuration() -> i64 {
    MEDIA_STATE.lock().unwrap().duration
}

#[no_mangle]
pub extern "system" fn isPlaying() -> bool {
    MEDIA_STATE.lock().unwrap().is_playing
}

#[no_mangle]
pub extern "system" fn getThumbnailPtr() -> *const u8 {
    MEDIA_STATE.lock().unwrap().thumbnail.as_ptr()
}

#[no_mangle]
pub extern "system" fn getThumbnailSize() -> usize {
    MEDIA_STATE.lock().unwrap().thumbnail.len()
}

fn control<F>(action: F)
where
    F: FnOnce(&GlobalSystemMediaTransportControlsSession),
{
    RT.block_on(async {
        let session = match get_session().await {
            Some(s) => s,
            None => return,
        };
        action(&session);
    });
}

#[no_mangle]
pub extern "system" fn play() {
    control(|s| {
        let _ = s.TryPlayAsync();
    });
}

#[no_mangle]
pub extern "system" fn pause() {
    control(|s| {
        let _ = s.TryPauseAsync();
    });
}

#[no_mangle]
pub extern "system" fn next() {
    control(|s| {
        let _ = s.TrySkipNextAsync();
    });
}

#[no_mangle]
pub extern "system" fn previous() {
    control(|s| {
        let _ = s.TrySkipPreviousAsync();
    });
}

#[no_mangle]
pub extern "system" fn free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(ptr);
    }
}
