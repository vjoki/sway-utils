use std::env;
use std::fs;
use std::net::Shutdown;
use std::path::PathBuf;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use swayipc::{Connection, EventType};
use swayipc::{Event, WindowChange};
use anyhow::{Context, Result, anyhow, bail};

/// Panics: on IO error.
fn cmd_server(socket_filename: PathBuf, prev_window: Arc<AtomicI64>) {
    if socket_filename.exists() {
        fs::remove_file(&socket_filename).expect("Unable to remove old socket file");
    }

    let listener = UnixListener::bind(&socket_filename).expect("Could not bind socket");
    let mut conn = Connection::new().expect("Could not obtain a connection to sway IPC socket");

    let mut buf: [u8; 4] = [0; 4];
    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            stream.read_exact(&mut buf).expect("Failed to read socket stream");
            match &buf {
                b"prev" => {
                    conn.run_command(format!("[con_id={}] focus", prev_window.load(Ordering::Acquire)))
                        .expect("Running sway IPC command failed");
                    let _ = stream.shutdown(Shutdown::Both);
                },
                _ => {
                    let _ = stream.shutdown(Shutdown::Both);
                    if socket_filename.exists() {
                        fs::remove_file(&socket_filename).expect("Unable to remove old socket file");
                    }
                    return;
                }
            }
        }
    }
}

fn focused_window(conn: &mut Connection) -> Result<i64> {
    let mut node = conn.get_tree()?;
    while !node.focused {
        let fid = node.focus.into_iter().next().ok_or_else(|| anyhow!("Sway tree has no focused nodes."))?;
        node = node.nodes.into_iter().chain(node.floating_nodes.into_iter())
            .find(|n| n.id == fid)
            .ok_or_else(|| anyhow!("Focused node not found in the nodes lists."))?;
    }
    Ok(node.id)
}

fn listen(socket_filename: PathBuf) -> Result<()> {
    let mut conn = Connection::new()?;
    let prev_window = Arc::new(AtomicI64::new(-1));
    let mut curr_window = focused_window(&mut conn).unwrap_or(-1);

    // Spawn unix socket listener.
    let prevc = Arc::clone(&prev_window);
    let socket_filenamec = socket_filename.clone();
    let listener_handle = thread::spawn(move || cmd_server(socket_filenamec, prevc));

    // Subscribe to sway window events.
    let events = conn.subscribe(&[EventType::Window, EventType::Shutdown])?;
    for event in events {
        // bail if
        if Arc::strong_count(&prev_window) < 2 {
            bail!("{} socket listener closed unexpectedly.", env!("CARGO_PKG_NAME"));
        }

        match event? {
            Event::Window(e) => {
                match e.change {
                    WindowChange::Focus => {
                        prev_window.store(curr_window, Ordering::Release);
                        curr_window = e.container.id;
                    },
                    WindowChange::Close => {
                        let _ = prev_window.compare_exchange(e.container.id, -1,
                                                             Ordering::Relaxed, Ordering::Relaxed);
                        if e.container.id == curr_window {
                            curr_window = -1;
                        }
                    },
                    _ => {}
                }
            },
            Event::Shutdown(_) => {
                UnixStream::connect(socket_filename)?.write_all(b"close")?;
                return listener_handle.join()
                    .map_err(|e| anyhow!("{} socket listener paniced: {:?}", env!("CARGO_PKG_NAME"), e));
            },
            _ => {}
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let socket_filename = PathBuf::from(format!("{}/sway-focus-switcheroo.socket", env::var("XDG_RUNTIME_DIR")?));

    if env::args().nth(1).is_some() {
        listen(socket_filename)
    } else {
        UnixStream::connect(socket_filename)
            .with_context(|| format!("Unable to connect {} socket.", env!("CARGO_PKG_NAME")))?
          .write_all(b"prev").map_err(|e| e.into())
    }
}
