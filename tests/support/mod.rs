use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::{Duration, Instant},
};

/// Stop only the caller's private fixture endpoint, after acquiring its controller.
/// Consume complete frames and retain the connection until shutdown is confirmed.
pub fn shutdown(socket: &Path) -> bool {
    let deadline = Instant::now() + Duration::from_secs(8);
    while socket.exists() && Instant::now() < deadline {
        if let Ok(mut stream) = UnixStream::connect(socket) {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
            if let Ok(copy) = stream.try_clone() {
                let mut reader = BufReader::new(copy);
                if frame(&mut reader).is_some_and(|state| state["control_denied"] == false)
                    && stream.write_all(b"\"Shutdown\"\n").is_ok()
                {
                    while Instant::now() < deadline {
                        let Some(state) = frame(&mut reader) else {
                            break;
                        };
                        if state["stopped"] == true {
                            while socket.exists() && Instant::now() < deadline {
                                std::thread::sleep(Duration::from_millis(10));
                            }
                            return !socket.exists();
                        }
                        if stream.write_all(b"\"Screen\"\n").is_err() {
                            break;
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    !socket.exists()
}

fn frame(reader: &mut BufReader<UnixStream>) -> Option<serde_json::Value> {
    const MAX_FRAME: usize = 4 * 1024 * 1024;
    let mut line = String::new();
    reader
        .take((MAX_FRAME + 1) as u64)
        .read_line(&mut line)
        .ok()?;
    if line.len() > MAX_FRAME || !line.ends_with('\n') {
        return None;
    }
    serde_json::from_str(&line).ok()
}
