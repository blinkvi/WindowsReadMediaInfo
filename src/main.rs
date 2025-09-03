use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};
use tokio::runtime::Runtime;
use std::fs::File;
use std::io::Write;
use tokio::time::{sleep, Duration};

fn main() -> windows::core::Result<()> {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let smtc: GlobalSystemMediaTransportControlsSessionManager =
            GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.await?;

        let mut last_title = String::new();
        let mut last_artist = String::new();
        let mut last_position_sec = -1i64;

        loop {
            let session = match smtc.GetCurrentSession() {
                Ok(s) => s,
                Err(_) => {
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };

            let media_props = match session.TryGetMediaPropertiesAsync()?.await {
                Ok(props) => props,
                Err(_) => {
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };

            let title = media_props.Title().unwrap_or_default();
            let artist = media_props.Artist().unwrap_or_default();

            if title != last_title {
                if let Ok(thumbnail_ref) = media_props.Thumbnail() {
                    if let Ok(stream) = IRandomAccessStreamReference::OpenReadAsync(&thumbnail_ref)?.await {
                        let size = stream.Size().unwrap_or(0) as usize;
                        let mut buffer = vec![0u8; size];
                        if let Ok(input_stream) = stream.GetInputStreamAt(0) {
                            if let Ok(reader) = DataReader::CreateDataReader(&input_stream) {
                                reader.LoadAsync(size as u32)?.await?;
                                let _ = reader.ReadBytes(&mut buffer);
                                let mut file = File::create("thumbnail.png")
                                    .map_err(|e| windows::core::Error::new(
                                        windows::core::HRESULT(0),
                                        e.to_string().into(),
                                    ))?;
                                file.write_all(&buffer)
                                    .map_err(|e| windows::core::Error::new(
                                        windows::core::HRESULT(0),
                                        e.to_string().into(),
                                    ))?;
                                println!("Thumbnail saved as thumbnail.png");
                            }
                        }
                    }
                }
            }

            let timeline = match session.GetTimelineProperties() {
                Ok(t) => t,
                Err(_) => {
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };

            let position_sec = (timeline.Position()?.Duration / 10_000_000) as i64;
            let duration_sec = ((timeline.EndTime()?.Duration - timeline.StartTime()?.Duration) / 10_000_000) as i64;

            if position_sec != last_position_sec || title != last_title || artist != last_artist {
                last_position_sec = position_sec;
                last_title = title.to_string();
                last_artist = artist.to_string();

                println!("Title: {}", title);
                println!("Artist: {}", artist);
                println!("Position: {}", format_time(position_sec));
                println!("Duration: {}", format_time(duration_sec));
            }

            sleep(Duration::from_millis(50)).await;
        }
    })
}

fn format_time(total_seconds: i64) -> String {
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}", minutes, seconds)
}
