use std::{
    io::Write,
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};

use crate::{
    config::NetworkConfig,
    protocol::{CameraFrame, GimbalCommand},
};

#[derive(Default)]
pub struct NetworkStats {
    pub commands_received: AtomicU64,
    pub telemetry_sent: AtomicU64,
    pub camera_frames_sent: AtomicU64,
    pub camera_clients: AtomicUsize,
}

#[derive(Resource)]
pub struct NetworkBridge {
    command_rx: Receiver<GimbalCommand>,
    telemetry_tx: Sender<String>,
    camera_tx: Sender<CameraFrame>,
    pub stats: Arc<NetworkStats>,
}

impl NetworkBridge {
    pub fn start(config: &NetworkConfig) -> Result<Self> {
        let (command_tx, command_rx) = unbounded();
        let (telemetry_tx, telemetry_rx) = bounded::<String>(8);
        let (camera_tx, camera_rx) = bounded::<CameraFrame>(2);
        let stats = Arc::new(NetworkStats::default());

        spawn_command_thread(config.command_bind.clone(), command_tx, stats.clone())?;
        spawn_telemetry_thread(
            config.telemetry_target.clone(),
            telemetry_rx,
            stats.clone(),
        )?;
        spawn_camera_thread(config.camera_bind.clone(), camera_rx, stats.clone())?;

        Ok(Self {
            command_rx,
            telemetry_tx,
            camera_tx,
            stats,
        })
    }

    pub fn drain_latest_command(&self) -> Option<GimbalCommand> {
        let mut latest = None;
        while let Ok(cmd) = self.command_rx.try_recv() {
            latest = Some(cmd);
        }
        latest
    }

    pub fn try_send_telemetry(&self, json: String) {
        let _ = self.telemetry_tx.try_send(json);
    }

    pub fn try_send_camera(&self, frame: CameraFrame) {
        let _ = self.camera_tx.try_send(frame);
    }
}

fn spawn_command_thread(
    bind: String,
    tx: Sender<GimbalCommand>,
    stats: Arc<NetworkStats>,
) -> Result<()> {
    let socket = UdpSocket::bind(&bind).with_context(|| format!("bind command UDP {bind}"))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(250)))
        .context("set command UDP timeout")?;
    thread::Builder::new()
        .name("aimsim-command-rx".into())
        .spawn(move || {
            let mut buf = [0u8; 2048];
            loop {
                match socket.recv_from(&mut buf) {
                    Ok((n, _)) => {
                        if let Ok(cmd) = serde_json::from_slice::<GimbalCommand>(&buf[..n]) {
                            stats.commands_received.fetch_add(1, Ordering::Relaxed);
                            let _ = tx.send(cmd);
                        }
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) => {}
                    Err(_) => thread::sleep(Duration::from_millis(10)),
                }
            }
        })
        .context("spawn command thread")?;
    Ok(())
}

fn spawn_telemetry_thread(
    target: String,
    rx: Receiver<String>,
    stats: Arc<NetworkStats>,
) -> Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0").context("bind telemetry UDP")?;
    thread::Builder::new()
        .name("aimsim-telemetry-tx".into())
        .spawn(move || {
            while let Ok(payload) = rx.recv() {
                if socket.send_to(payload.as_bytes(), &target).is_ok() {
                    stats.telemetry_sent.fetch_add(1, Ordering::Relaxed);
                }
            }
        })
        .context("spawn telemetry thread")?;
    Ok(())
}

fn spawn_camera_thread(
    bind: String,
    rx: Receiver<CameraFrame>,
    stats: Arc<NetworkStats>,
) -> Result<()> {
    let listener = TcpListener::bind(&bind).with_context(|| format!("bind camera TCP {bind}"))?;
    listener
        .set_nonblocking(true)
        .context("set camera TCP nonblocking")?;

    thread::Builder::new()
        .name("aimsim-camera-tx".into())
        .spawn(move || {
            let mut clients: Vec<TcpStream> = Vec::new();
            loop {
                loop {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let _ = stream.set_nodelay(true);
                            clients.push(stream);
                            stats.camera_clients.store(clients.len(), Ordering::Relaxed);
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(_) => break,
                    }
                }

                match rx.recv_timeout(Duration::from_millis(20)) {
                    Ok(frame) => {
                        let packet = frame.encode();
                        clients.retain_mut(|client| client.write_all(&packet).is_ok());
                        stats.camera_clients.store(clients.len(), Ordering::Relaxed);
                        if !clients.is_empty() {
                            stats.camera_frames_sent.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .context("spawn camera thread")?;
    Ok(())
}
